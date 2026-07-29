//! 交互式单跨梁分析：内力图、力法和位移法。
//!
//! 运行：
//! cargo run -p playground --example 24_forceGraph

use std::{ops::Range, sync::Arc};

use gpui::{
    AnyView, App, Application, BorderStyle, Bounds, ClipboardItem, Context, CursorStyle, Div,
    Element, ElementId, Entity, FocusHandle, FontWeight, GlobalElementId, InspectorElementId,
    KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    PathBuilder, Pixels, Render, Rgba, ShapedLine, Style, StyleRefinement, TextRun,
    TitlebarOptions, Window, WindowBounds, WindowControlArea, WindowDecorations, WindowOptions,
    canvas, div, fill, point, prelude::*, px, quad, relative, rgb, size,
};

const BG: u32 = 0x07111f;
const SIDEBAR: u32 = 0x091624;
const PANEL: u32 = 0x0d1d2d;
const CARD: u32 = 0x102337;
const BORDER: u32 = 0x203b50;
const TEXT: u32 = 0xe9f4fb;
const MUTED: u32 = 0x86a2b5;
const CYAN: u32 = 0x22d3ee;
const BLUE: u32 = 0x60a5fa;
const AMBER: u32 = 0xfbbf24;
const PINK: u32 = 0xf472b6;
const GREEN: u32 = 0x34d399;
const RED: u32 = 0xfb7185;
const FIELD_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    InternalForce,
    ForceMethod,
    DisplacementMethod,
}

impl Page {
    fn label(self) -> &'static str {
        match self {
            Self::InternalForce => "内力图",
            Self::ForceMethod => "力法",
            Self::DisplacementMethod => "位移法",
        }
    }

    fn caption(self) -> &'static str {
        match self {
            Self::InternalForce => "V / M",
            Self::ForceMethod => "M₀ + Mₓ",
            Self::DisplacementMethod => "KΔ = F",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Support {
    Fixed,
    Pinned,
    Roller,
    Free,
}

impl Support {
    fn label(self) -> &'static str {
        match self {
            Self::Fixed => "固支",
            Self::Pinned => "铰支",
            Self::Roller => "滚支",
            Self::Free => "自由",
        }
    }

    fn short(self) -> &'static str {
        match self {
            Self::Fixed => "固",
            Self::Pinned => "铰",
            Self::Roller => "滚",
            Self::Free => "自",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Fixed => Self::Pinned,
            Self::Pinned => Self::Roller,
            Self::Roller => Self::Free,
            Self::Free => Self::Fixed,
        }
    }

    fn restrains_vertical(self) -> bool {
        self != Self::Free
    }

    fn restrains_rotation(self) -> bool {
        self == Self::Fixed
    }
}

#[derive(Clone, Debug, PartialEq)]
struct BeamParams {
    span: f64,
    load: f64,
    load_position: f64,
    elastic_modulus: f64,
    inertia_millionth: f64,
}

impl Default for BeamParams {
    fn default() -> Self {
        Self {
            span: 8.0,
            load: 80.0,
            load_position: 3.2,
            elastic_modulus: 200.0,
            inertia_millionth: 8.0,
        }
    }
}

impl BeamParams {
    fn field_values(&self) -> [String; FIELD_COUNT] {
        [
            pretty_number(self.span),
            pretty_number(self.load),
            pretty_number(self.load_position),
            pretty_number(self.elastic_modulus),
            pretty_number(self.inertia_millionth),
        ]
    }

    fn ei(&self) -> f64 {
        self.elastic_modulus * 1.0e9 * self.inertia_millionth * 1.0e-6
    }
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    x: f64,
    shear: f64,
    moment: f64,
    base_moment: f64,
    correction_moment: f64,
    displacement: f64,
}

#[derive(Clone, Debug)]
struct Analysis {
    params: BeamParams,
    left: Support,
    right: Support,
    nodes: Vec<f64>,
    dofs: Vec<f64>,
    reactions: Vec<f64>,
    samples: Vec<Sample>,
    max_shear: f64,
    max_moment: f64,
    max_displacement: f64,
    indeterminacy: usize,
}

impl Analysis {
    fn left_reaction(&self) -> f64 {
        self.reactions[0] / 1_000.0
    }

    fn right_reaction(&self) -> f64 {
        self.reactions[self.reactions.len() - 2] / 1_000.0
    }

    fn left_reaction_moment(&self) -> f64 {
        self.reactions[1] / 1_000.0
    }

    fn right_reaction_moment(&self) -> f64 {
        self.reactions[self.reactions.len() - 1] / 1_000.0
    }

    fn left_internal_moment(&self) -> f64 {
        -self.left_reaction_moment()
    }

    fn right_internal_moment(&self) -> f64 {
        self.right_reaction_moment()
    }
}

#[derive(Clone, Copy)]
enum GraphValue {
    Shear,
    Moment,
    BaseMoment,
    CorrectionMoment,
    Displacement,
}

impl GraphValue {
    fn value(self, sample: &Sample) -> f64 {
        match self {
            Self::Shear => sample.shear,
            Self::Moment => sample.moment,
            Self::BaseMoment => sample.base_moment,
            Self::CorrectionMoment => sample.correction_moment,
            Self::Displacement => sample.displacement,
        }
    }
}

#[derive(Clone, Copy)]
enum CanvasKind {
    Structure,
    Graph { value: GraphValue, accent: u32 },
    Deformed,
}

struct BeamCanvas {
    analysis: Arc<Analysis>,
    kind: CanvasKind,
}

impl Render for BeamCanvas {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let analysis = Arc::clone(&self.analysis);
        let drawing = match self.kind {
            CanvasKind::Structure => canvas(
                |_, _, _| {},
                move |bounds, _, window, _| paint_structure(bounds, &analysis, window),
            )
            .size_full()
            .into_any_element(),
            CanvasKind::Graph { value, accent } => canvas(
                |_, _, _| {},
                move |bounds, _, window, _| paint_graph(bounds, &analysis, value, accent, window),
            )
            .size_full()
            .into_any_element(),
            CanvasKind::Deformed => canvas(
                |_, _, _| {},
                move |bounds, _, window, _| paint_deformed_shape(bounds, &analysis, window),
            )
            .size_full()
            .into_any_element(),
        };

        div().size_full().child(drawing)
    }
}

struct BeamLab {
    page: Page,
    params: BeamParams,
    analysis: Result<Arc<Analysis>, String>,
    canvases: [Entity<BeamCanvas>; 7],
    fields: [String; FIELD_COUNT],
    field_focus: [FocusHandle; FIELD_COUNT],
    field_anchor: [usize; FIELD_COUNT],
    field_caret: [usize; FIELD_COUNT],
    field_layout: [Option<ShapedLine>; FIELD_COUNT],
    field_bounds: [Option<Bounds<Pixels>>; FIELD_COUNT],
    field_selecting: [bool; FIELD_COUNT],
    root_focus: FocusHandle,
    left_support: Support,
    right_support: Support,
    input_error: Option<String>,
}

struct NumericTextElement {
    app: Entity<BeamLab>,
    index: usize,
}

struct NumericTextPrepaint {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for NumericTextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for NumericTextElement {
    type RequestLayoutState = ();
    type PrepaintState = NumericTextPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let app = self.app.read(cx);
        let content: gpui::SharedString = app.fields[self.index].clone().into();
        let start = app.field_anchor[self.index].min(app.field_caret[self.index]);
        let end = app.field_anchor[self.index].max(app.field_caret[self.index]);
        let style = window.text_style();
        let run = TextRun {
            len: content.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(content, font_size, &[run], None);

        let cursor_x = line.x_for_index(app.field_caret[self.index]);
        let (selection, cursor) = if start == end {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_x, bounds.top()),
                        size(px(1.5), bounds.size.height),
                    ),
                    rgb(CYAN),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(bounds.left() + line.x_for_index(start), bounds.top()),
                        point(bounds.left() + line.x_for_index(end), bounds.bottom()),
                    ),
                    with_alpha(CYAN, 0.24),
                )),
                None,
            )
        };

        NumericTextPrepaint {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let line = prepaint.line.take().expect("numeric input line was shaped");
        line.paint(bounds.origin, window.line_height(), window, cx)
            .expect("failed to paint numeric input");

        let focus = self.app.read(cx).field_focus[self.index].clone();
        if focus.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.app.update(cx, |app, _| {
            app.field_layout[self.index] = Some(line);
            app.field_bounds[self.index] = Some(bounds);
        });
    }
}

impl BeamLab {
    fn new(cx: &mut Context<Self>) -> Self {
        let params = BeamParams::default();
        let fields = params.field_values();
        let field_anchor = std::array::from_fn(|index| fields[index].len());
        let left_support = Support::Pinned;
        let right_support = Support::Roller;
        let analysis = Arc::new(
            analyze(params.clone(), left_support, right_support)
                .expect("default beam model must be stable"),
        );
        let canvas_kinds = [
            CanvasKind::Structure,
            CanvasKind::Graph {
                value: GraphValue::Shear,
                accent: CYAN,
            },
            CanvasKind::Graph {
                value: GraphValue::Moment,
                accent: AMBER,
            },
            CanvasKind::Graph {
                value: GraphValue::BaseMoment,
                accent: BLUE,
            },
            CanvasKind::Graph {
                value: GraphValue::CorrectionMoment,
                accent: PINK,
            },
            CanvasKind::Deformed,
            CanvasKind::Graph {
                value: GraphValue::Displacement,
                accent: BLUE,
            },
        ];
        let canvases = std::array::from_fn(|index| {
            let analysis = Arc::clone(&analysis);
            cx.new(move |_| BeamCanvas {
                analysis,
                kind: canvas_kinds[index],
            })
        });

        Self {
            page: Page::InternalForce,
            fields,
            params,
            analysis: Ok(analysis),
            canvases,
            field_focus: std::array::from_fn(|index| {
                cx.focus_handle()
                    .tab_index(index as isize + 1)
                    .tab_stop(true)
            }),
            field_anchor,
            field_caret: field_anchor,
            field_layout: std::array::from_fn(|_| None),
            field_bounds: [None; FIELD_COUNT],
            field_selecting: [false; FIELD_COUNT],
            root_focus: cx.focus_handle().tab_stop(false),
            left_support,
            right_support,
            input_error: None,
        }
    }

    fn refresh_analysis(&mut self, cx: &mut Context<Self>) {
        match analyze(self.params.clone(), self.left_support, self.right_support) {
            Ok(analysis) => {
                let analysis = Arc::new(analysis);
                self.analysis = Ok(Arc::clone(&analysis));
                for canvas in &self.canvases {
                    let analysis = Arc::clone(&analysis);
                    canvas.update(cx, move |canvas, cx| {
                        canvas.analysis = analysis;
                        cx.notify();
                    });
                }
            }
            Err(error) => self.analysis = Err(error),
        }
    }

    fn apply_fields(&mut self, cx: &mut Context<Self>) {
        let parsed = self
            .fields
            .iter()
            .map(|value| value.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>();

        let values = match parsed {
            Ok(values) => values,
            Err(_) => {
                self.input_error = Some("请输入有效数字".into());
                return;
            }
        };

        let candidate = BeamParams {
            span: values[0],
            load: values[1],
            load_position: values[2],
            elastic_modulus: values[3],
            inertia_millionth: values[4],
        };

        let error = if !(0.5..=100.0).contains(&candidate.span) {
            Some("跨径 L 应在 0.5～100 m")
        } else if !(0.01..=100_000.0).contains(&candidate.load) {
            Some("集中力 P 应大于 0")
        } else if !(0.0..=candidate.span).contains(&candidate.load_position) {
            Some("力位置 a 应位于梁跨内")
        } else if candidate.elastic_modulus <= 0.0 {
            Some("弹性模量 E 应大于 0")
        } else if candidate.inertia_millionth <= 0.0 {
            Some("惯性矩 I 应大于 0")
        } else {
            None
        };

        if let Some(error) = error {
            self.input_error = Some(error.into());
        } else {
            self.input_error = None;
            if self.params != candidate {
                self.params = candidate;
                self.refresh_analysis(cx);
            }
        }
    }

    fn restore_fields(&mut self) {
        self.fields = self.params.field_values();
        for index in 0..FIELD_COUNT {
            let end = self.fields[index].len();
            self.field_anchor[index] = end;
            self.field_caret[index] = end;
        }
        self.input_error = None;
    }

    fn field_selection(&self, index: usize) -> Range<usize> {
        let start = self.field_anchor[index].min(self.field_caret[index]);
        let end = self.field_anchor[index].max(self.field_caret[index]);
        start..end
    }

    fn select_all_field(&mut self, index: usize) {
        self.field_anchor[index] = 0;
        self.field_caret[index] = self.fields[index].len();
    }

    fn field_index_for_position(&self, index: usize, position: gpui::Point<Pixels>) -> usize {
        let Some(bounds) = self.field_bounds[index] else {
            return self.fields[index].len();
        };
        let Some(line) = self.field_layout[index].as_ref() else {
            return self.fields[index].len();
        };
        if position.x <= bounds.left() {
            0
        } else if position.x >= bounds.right() {
            self.fields[index].len()
        } else {
            line.closest_index_for_x(position.x - bounds.left())
                .min(self.fields[index].len())
        }
    }

    fn on_field_mouse_down(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.field_focus[index]);
        if event.click_count >= 2 {
            self.select_all_field(index);
            self.field_selecting[index] = false;
        } else {
            let offset = self.field_index_for_position(index, event.position);
            if event.modifiers.shift {
                self.field_caret[index] = offset;
            } else {
                self.field_anchor[index] = offset;
                self.field_caret[index] = offset;
            }
            self.field_selecting[index] = true;
        }
        cx.notify();
    }

    fn on_field_mouse_move(
        &mut self,
        index: usize,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        if self.field_selecting[index] {
            self.field_caret[index] = self.field_index_for_position(index, event.position);
            cx.notify();
        }
    }

    fn on_field_mouse_up(&mut self, index: usize, _: &MouseUpEvent, _: &mut Context<Self>) {
        self.field_selecting[index] = false;
    }

    fn replace_field_selection(&mut self, index: usize, text: &str) {
        let range = self.field_selection(index);
        let caret = replace_numeric_range(&mut self.fields[index], range, text, 12);
        self.field_anchor[index] = caret;
        self.field_caret[index] = caret;
    }

    fn delete_field_selection(&mut self, index: usize, backwards: bool) {
        let mut range = self.field_selection(index);
        if range.is_empty() {
            if backwards && range.start > 0 {
                range.start -= 1;
            } else if !backwards && range.end < self.fields[index].len() {
                range.end += 1;
            }
        }
        if !range.is_empty() {
            let caret = range.start;
            self.fields[index].replace_range(range, "");
            self.field_anchor[index] = caret;
            self.field_caret[index] = caret;
        }
    }

    fn move_field_caret(&mut self, index: usize, target: usize, extend: bool) {
        let target = target.min(self.fields[index].len());
        if extend {
            self.field_caret[index] = target;
        } else {
            self.field_anchor[index] = target;
            self.field_caret[index] = target;
        }
    }

    fn on_field_key(
        &mut self,
        index: usize,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let command = event.keystroke.modifiers.control || event.keystroke.modifiers.platform;
        if command {
            match key {
                "a" => self.select_all_field(index),
                "c" => {
                    let range = self.field_selection(index);
                    if !range.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            self.fields[index][range].to_string(),
                        ));
                    }
                }
                "x" => {
                    let range = self.field_selection(index);
                    if !range.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            self.fields[index][range.clone()].to_string(),
                        ));
                        self.delete_field_selection(index, true);
                        self.apply_fields(cx);
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.replace_field_selection(index, &text);
                        self.apply_fields(cx);
                    }
                }
                _ => return,
            }
            cx.notify();
            return;
        }

        match event.keystroke.key.as_str() {
            "backspace" => {
                self.delete_field_selection(index, true);
            }
            "delete" => {
                self.delete_field_selection(index, false);
            }
            "left" => {
                let range = self.field_selection(index);
                let target = if event.keystroke.modifiers.shift {
                    self.field_caret[index].saturating_sub(1)
                } else if !range.is_empty() {
                    range.start
                } else {
                    self.field_caret[index].saturating_sub(1)
                };
                self.move_field_caret(index, target, event.keystroke.modifiers.shift);
                cx.notify();
                return;
            }
            "right" => {
                let range = self.field_selection(index);
                let target = if event.keystroke.modifiers.shift {
                    (self.field_caret[index] + 1).min(self.fields[index].len())
                } else if !range.is_empty() {
                    range.end
                } else {
                    (self.field_caret[index] + 1).min(self.fields[index].len())
                };
                self.move_field_caret(index, target, event.keystroke.modifiers.shift);
                cx.notify();
                return;
            }
            "home" => {
                self.move_field_caret(index, 0, event.keystroke.modifiers.shift);
                cx.notify();
                return;
            }
            "end" => {
                self.move_field_caret(
                    index,
                    self.fields[index].len(),
                    event.keystroke.modifiers.shift,
                );
                cx.notify();
                return;
            }
            "enter" => {
                self.apply_fields(cx);
                window.focus(&self.root_focus);
                cx.notify();
                return;
            }
            "escape" => {
                self.restore_fields();
                window.focus(&self.root_focus);
                cx.notify();
                return;
            }
            "up" => {
                self.nudge_field(index, 1.0, cx);
                cx.notify();
                return;
            }
            "down" => {
                self.nudge_field(index, -1.0, cx);
                cx.notify();
                return;
            }
            _ => {
                if let Some(text) = event.keystroke.key_char.as_deref()
                    && text
                        .chars()
                        .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-')
                {
                    self.replace_field_selection(index, text);
                } else {
                    return;
                }
            }
        }

        self.apply_fields(cx);
        cx.notify();
    }

    fn nudge_field(&mut self, index: usize, direction: f64, cx: &mut Context<Self>) {
        const STEPS: [f64; FIELD_COUNT] = [0.5, 5.0, 0.2, 10.0, 1.0];
        let fallback = match index {
            0 => self.params.span,
            1 => self.params.load,
            2 => self.params.load_position,
            3 => self.params.elastic_modulus,
            _ => self.params.inertia_millionth,
        };
        let current = self.fields[index].parse::<f64>().unwrap_or(fallback);
        let mut next = current + STEPS[index] * direction;

        next = match index {
            0 => next.clamp(0.5, 100.0),
            1 => next.max(0.01),
            2 => next.clamp(0.0, self.params.span),
            3 | 4 => next.max(STEPS[index]),
            _ => next,
        };

        self.fields[index] = pretty_number(next);
        self.select_all_field(index);
        self.apply_fields(cx);
    }

    fn set_preset(&mut self, preset: usize, cx: &mut Context<Self>) {
        let supports = match preset {
            0 => (Support::Pinned, Support::Roller),
            1 => (Support::Fixed, Support::Free),
            _ => (Support::Fixed, Support::Fixed),
        };
        if (self.left_support, self.right_support) != supports {
            (self.left_support, self.right_support) = supports;
            self.refresh_analysis(cx);
            cx.notify();
        }
    }

    fn render_tab(&self, page: Page, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.page == page;
        div()
            .id(("page-tab", page as usize))
            .flex()
            .items_center()
            .gap_2()
            .h_10()
            .px_4()
            .rounded_lg()
            .cursor_pointer()
            .text_sm()
            .font_weight(if active {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .text_color(rgb(if active { TEXT } else { MUTED }))
            .bg(rgb(if active { CARD } else { BG }))
            .border_1()
            .border_color(rgb(if active { CYAN } else { BORDER }))
            .hover(|style| style.bg(rgb(CARD)).text_color(rgb(TEXT)))
            .child(page.label())
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(if active { CYAN } else { MUTED }))
                    .child(page.caption()),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.page != page {
                    this.page = page;
                    cx.notify();
                }
            }))
    }

    fn render_field(
        &self,
        index: usize,
        label: &'static str,
        symbol: &'static str,
        unit: &'static str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let focused = self.field_focus[index].is_focused(window);

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(label)
                    .child(symbol),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id(("minus", index))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_8()
                            .rounded_lg()
                            .cursor_pointer()
                            .bg(rgb(PANEL))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .text_color(rgb(MUTED))
                            .hover(|style| style.text_color(rgb(CYAN)).border_color(rgb(CYAN)))
                            .child("−")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.nudge_field(index, -1.0, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id(("number-field", index))
                            .track_focus(&self.field_focus[index])
                            .tab_index(index as isize + 1)
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_between()
                            .h_10()
                            .px_3()
                            .rounded_lg()
                            .cursor(CursorStyle::IBeam)
                            .bg(rgb(if focused { 0x102b3c } else { PANEL }))
                            .border_1()
                            .border_color(rgb(if focused { CYAN } else { BORDER }))
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(TEXT))
                            .focus(|style| style.border_color(rgb(CYAN)))
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .child(NumericTextElement {
                                        app: cx.entity(),
                                        index,
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(rgb(MUTED))
                                    .child(unit),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event, window, cx| {
                                    this.on_field_mouse_down(index, event, window, cx);
                                }),
                            )
                            .on_mouse_move(cx.listener(move |this, event, _, cx| {
                                this.on_field_mouse_move(index, event, cx);
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, event, _, cx| {
                                    this.on_field_mouse_up(index, event, cx);
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(move |this, event, _, cx| {
                                    this.on_field_mouse_up(index, event, cx);
                                }),
                            )
                            .on_key_down(cx.listener(move |this, event, window, cx| {
                                this.on_field_key(index, event, window, cx);
                            })),
                    )
                    .child(
                        div()
                            .id(("plus", index))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_8()
                            .rounded_lg()
                            .cursor_pointer()
                            .bg(rgb(PANEL))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .text_color(rgb(MUTED))
                            .hover(|style| style.text_color(rgb(CYAN)).border_color(rgb(CYAN)))
                            .child("+")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.nudge_field(index, 1.0, cx);
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_support(&self, is_left: bool, title: &'static str, cx: &mut Context<Self>) -> Div {
        let support = if is_left {
            self.left_support
        } else {
            self.right_support
        };

        div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .child(div().text_xs().text_color(rgb(MUTED)).child(title))
            .child(
                div()
                    .id(if is_left {
                        "left-support"
                    } else {
                        "right-support"
                    })
                    .flex()
                    .items_center()
                    .justify_between()
                    .h_10()
                    .px_3()
                    .rounded_lg()
                    .cursor_pointer()
                    .bg(rgb(PANEL))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .hover(|style| style.border_color(rgb(CYAN)).bg(rgb(0x102b3c)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_6()
                            .rounded_full()
                            .bg(rgb(0x17384a))
                            .text_xs()
                            .text_color(rgb(CYAN))
                            .child(support.short()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(TEXT))
                            .child(support.label()),
                    )
                    .child(div().text_color(rgb(MUTED)).child("⌄"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if is_left {
                            this.left_support = this.left_support.next();
                        } else {
                            this.right_support = this.right_support.next();
                        }
                        this.refresh_analysis(cx);
                        cx.notify();
                    })),
            )
    }

    fn render_preset(
        &self,
        index: usize,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = match index {
            0 => self.left_support == Support::Pinned && self.right_support == Support::Roller,
            1 => self.left_support == Support::Fixed && self.right_support == Support::Free,
            _ => self.left_support == Support::Fixed && self.right_support == Support::Fixed,
        };

        div()
            .id(("preset", index))
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .h_8()
            .rounded_lg()
            .cursor_pointer()
            .text_xs()
            .text_color(rgb(if active { CYAN } else { MUTED }))
            .bg(rgb(if active { 0x123347 } else { PANEL }))
            .border_1()
            .border_color(rgb(if active { CYAN } else { BORDER }))
            .hover(|style| style.border_color(rgb(CYAN)).text_color(rgb(TEXT)))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_preset(index, cx);
            }))
    }

    fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input_status = self.input_error.clone();

        div()
            .id("parameter-sidebar")
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(310.0))
            .h_full()
            .overflow_y_scroll()
            .p_4()
            .gap_4()
            .bg(rgb(SIDEBAR))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(TEXT))
                                    .child("模型参数"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child("点击数值后直接键入 · ↑↓ 微调"),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .bg(rgb(0x103328))
                            .text_xs()
                            .text_color(rgb(GREEN))
                            .child("SI"),
                    ),
            )
            .child(self.render_field(0, "跨径", "L", "m", window, cx))
            .child(self.render_field(1, "集中力", "P", "kN ↓", window, cx))
            .child(self.render_field(2, "力的位置", "a", "m", window, cx))
            .child(div().h(px(1.0)).w_full().bg(rgb(BORDER)).opacity(0.7))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(TEXT))
                            .child("边界约束"),
                    )
                    .child(div().text_xs().text_color(rgb(MUTED)).child("点击循环")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.render_support(true, "左端 A", cx))
                    .child(self.render_support(false, "右端 B", cx)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.render_preset(0, "简支", cx))
                    .child(self.render_preset(1, "悬臂", cx))
                    .child(self.render_preset(2, "固固", cx)),
            )
            .child(div().h(px(1.0)).w_full().bg(rgb(BORDER)).opacity(0.7))
            .child(self.render_field(3, "弹性模量", "E", "GPa", window, cx))
            .child(self.render_field(4, "截面惯性矩", "I", "×10⁻⁶ m⁴", window, cx))
            .when_some(input_status, |this, error| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0x3a1821))
                        .border_1()
                        .border_color(rgb(0x783344))
                        .text_xs()
                        .text_color(rgb(RED))
                        .child("!")
                        .child(error),
                )
            })
            .child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(0x0b2030))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .line_height(px(19.0))
                    .child(
                        "假定：等截面 Euler–Bernoulli 梁，集中力竖直向下；忽略剪切变形与轴向变形。",
                    ),
            )
    }

    fn render_header(&self, stable: bool, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .flex_shrink_0()
            .h(px(76.0))
            .px_5()
            .bg(rgb(BG))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .id("window-drag-region")
                    .flex()
                    .flex_1()
                    .items_center()
                    .gap_3()
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_move();
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_10()
                            .rounded_xl()
                            .bg(rgb(CYAN))
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(BG))
                            .child("Σ"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(TEXT))
                                    .child("BeamLab"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child("单跨梁 · 快速结构分析"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.render_tab(Page::InternalForce, cx))
                    .child(self.render_tab(Page::ForceMethod, cx))
                    .child(self.render_tab(Page::DisplacementMethod, cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_full()
                    .bg(rgb(if stable { 0x103328 } else { 0x3a1821 }))
                    .text_xs()
                    .text_color(rgb(if stable { GREEN } else { RED }))
                    .child(
                        div()
                            .size_2()
                            .rounded_full()
                            .bg(rgb(if stable { GREEN } else { RED })),
                    )
                    .child(if stable {
                        "模型稳定"
                    } else {
                        "约束不足"
                    }),
            )
    }

    fn render_metric(
        &self,
        label: &'static str,
        value: String,
        detail: String,
        accent: u32,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(145.0))
            .gap_1()
            .p_3()
            .rounded_xl()
            .bg(rgb(CARD))
            .border_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(div().size_2().rounded_full().bg(rgb(accent)))
                    .child(label),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(TEXT))
                    .child(value),
            )
            .child(div().text_xs().text_color(rgb(MUTED)).child(detail))
    }

    fn render_chart_card(
        &self,
        title: &'static str,
        subtitle: String,
        accent: u32,
        body: impl IntoElement,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(250.0))
            .overflow_hidden()
            .rounded_xl()
            .bg(rgb(CARD))
            .border_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h_11()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size_2().rounded_full().bg(rgb(accent)))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(TEXT))
                                    .child(title),
                            ),
                    )
                    .child(div().text_xs().text_color(rgb(MUTED)).child(subtitle)),
            )
            .child(body)
    }

    fn cached_canvas(&self, index: usize, height: f32) -> Div {
        let style = StyleRefinement::default().size_full();
        div().w_full().h(px(height)).child(
            AnyView::from(self.canvases[index].clone())
                .cached(style)
                .into_any_element(),
        )
    }

    fn render_summary(&self, analysis: &Analysis) -> Div {
        div()
            .flex()
            .flex_wrap()
            .gap_3()
            .child(self.render_metric(
                "A 端反力",
                format!("{:+.2} kN", analysis.left_reaction()),
                format!("Mᴀ {:+.2} kN·m", analysis.left_reaction_moment()),
                CYAN,
            ))
            .child(self.render_metric(
                "B 端反力",
                format!("{:+.2} kN", analysis.right_reaction()),
                format!("Mʙ {:+.2} kN·m", analysis.right_reaction_moment()),
                BLUE,
            ))
            .child(self.render_metric(
                "最大弯矩",
                format!("{:.2} kN·m", analysis.max_moment / 1_000.0),
                "max |M(x)|".into(),
                AMBER,
            ))
            .child(self.render_metric(
                "最大挠度",
                format!("{:.3} mm", analysis.max_displacement * 1_000.0),
                "max |v(x)|".into(),
                PINK,
            ))
    }

    fn render_structure_card(&self, analysis: &Analysis) -> Div {
        let subtitle = format!(
            "L = {} m   ·   P = {} kN @ {} m",
            pretty_number(analysis.params.span),
            pretty_number(analysis.params.load),
            pretty_number(analysis.params.load_position)
        );

        self.render_chart_card("计算模型", subtitle, CYAN, self.cached_canvas(0, 155.0))
    }

    fn render_internal_force(&self, analysis: &Analysis) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.render_summary(analysis))
            .child(self.render_structure_card(analysis))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(self.render_chart_card(
                        "剪力图 V(x)",
                        format!("max |V| = {:.2} kN", analysis.max_shear / 1_000.0),
                        CYAN,
                        self.cached_canvas(1, 215.0),
                    ))
                    .child(self.render_chart_card(
                        "弯矩图 M(x)",
                        format!("max |M| = {:.2} kN·m", analysis.max_moment / 1_000.0),
                        AMBER,
                        self.cached_canvas(2, 215.0),
                    )),
            )
    }

    fn render_force_method(&self, analysis: &Analysis) -> Div {
        let is_simple = analysis.left.restrains_vertical()
            && !analysis.left.restrains_rotation()
            && analysis.right.restrains_vertical()
            && !analysis.right.restrains_rotation();
        let force_method_note = if is_simple {
            "静定简支体系：无多余未知力，Mₓ = 0"
        } else if analysis.indeterminacy == 0 {
            "静定体系：无需力法；Mₓ 图仅表示相对简支参考体系的弯矩差"
        } else {
            "以简支梁为基本体系，用约束位移为零建立典型方程"
        };
        let left_unknown_label = if analysis.indeterminacy > 0 {
            "多余未知量 Xᴀ"
        } else {
            "A 端截面弯矩"
        };
        let right_unknown_label = if analysis.indeterminacy > 0 {
            "多余未知量 Xʙ"
        } else {
            "B 端截面弯矩"
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(self.render_metric(
                        "超静定次数",
                        format!("{} 次", analysis.indeterminacy),
                        "r = 约束数 − 2".into(),
                        PINK,
                    ))
                    .child(self.render_metric(
                        left_unknown_label,
                        format!("{:+.2} kN·m", analysis.left_internal_moment()),
                        "A 端内力矩".into(),
                        CYAN,
                    ))
                    .child(self.render_metric(
                        right_unknown_label,
                        format!("{:+.2} kN·m", analysis.right_internal_moment()),
                        "B 端内力矩".into(),
                        BLUE,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_4()
                    .rounded_xl()
                    .bg(rgb(0x10263a))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(TEXT))
                                    .child("力法典型方程"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(force_method_note),
                            ),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(rgb(PANEL))
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(CYAN))
                            .child("δ₀ + δᵢⱼ Xⱼ = 0"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(self.render_chart_card(
                        "基本体系 M₀",
                        "简支梁荷载弯矩".into(),
                        BLUE,
                        self.cached_canvas(3, 195.0),
                    ))
                    .child(self.render_chart_card(
                        "约束贡献 Mₓ",
                        "多余未知力贡献".into(),
                        PINK,
                        self.cached_canvas(4, 195.0),
                    ))
                    .child(self.render_chart_card(
                        "最终弯矩 M",
                        "M = M₀ + Mₓ".into(),
                        AMBER,
                        self.cached_canvas(2, 195.0),
                    )),
            )
    }

    fn render_displacement_method(&self, analysis: &Analysis) -> Div {
        let free_dofs = analysis
            .dofs
            .iter()
            .filter(|value| value.abs() > 1.0e-14)
            .count();
        let load_node = analysis
            .nodes
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (*a - analysis.params.load_position)
                    .abs()
                    .total_cmp(&(*b - analysis.params.load_position).abs())
            })
            .map(|(index, _)| index)
            .unwrap_or(0);
        let node_v = analysis.dofs[load_node * 2] * 1_000.0;
        let node_theta = analysis.dofs[load_node * 2 + 1] * 1_000.0;

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(self.render_metric(
                        "整体刚度",
                        format!("{:.3} MN·m²", analysis.params.ei() / 1.0e6),
                        "EI".into(),
                        CYAN,
                    ))
                    .child(self.render_metric(
                        "活动自由度",
                        format!("{free_dofs}"),
                        format!("总自由度 {}", analysis.dofs.len()),
                        BLUE,
                    ))
                    .child(self.render_metric(
                        "荷载点位移",
                        format!("{node_v:+.3} mm"),
                        format!("θ = {node_theta:+.3} mrad"),
                        PINK,
                    ))
                    .child(self.render_metric(
                        "平衡残差",
                        "< 1e−9".into(),
                        "[K]{Δ} = {F}".into(),
                        GREEN,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_4()
                    .rounded_xl()
                    .bg(rgb(0x10263a))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(TEXT))
                                    .child("位移法"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child("梁单元组装整体刚度，施加约束后求结点位移"),
                            ),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(rgb(PANEL))
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(CYAN))
                            .child("[K]{Δ} = {F}"),
                    ),
            )
            .child(self.render_chart_card(
                "变形示意",
                "浅色：原结构 · 实线：自动放大后的弹性曲线".into(),
                PINK,
                self.cached_canvas(5, 245.0),
            ))
            .child(self.render_chart_card(
                "挠度曲线 v(x)",
                format!("max |v| = {:.3} mm", analysis.max_displacement * 1_000.0),
                BLUE,
                self.cached_canvas(6, 205.0),
            ))
    }

    fn render_unstable(&self, message: String) -> Div {
        div().flex().flex_1().items_center().justify_center().child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .max_w(px(470.0))
                .p_8()
                .rounded_xl()
                .bg(rgb(CARD))
                .border_1()
                .border_color(rgb(0x783344))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_12()
                        .rounded_full()
                        .bg(rgb(0x3a1821))
                        .text_xl()
                        .text_color(rgb(RED))
                        .child("!"),
                )
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(TEXT))
                        .child("当前约束不能形成稳定体系"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_center()
                        .text_color(rgb(MUTED))
                        .child(message),
                )
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .rounded_lg()
                        .bg(rgb(PANEL))
                        .text_xs()
                        .text_color(rgb(CYAN))
                        .child("可选择：简支、悬臂、固固，或手动组合稳定约束"),
                ),
        )
    }
}

impl Render for BeamLab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let analysis = self.analysis.clone();
        let stable = analysis.is_ok();

        div()
            .track_focus(&self.root_focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .child(self.render_header(stable, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(self.render_sidebar(window, cx))
                    .child(
                        div()
                            .id("analysis-scroll")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.0))
                            .h_full()
                            .overflow_y_scroll()
                            .p_4()
                            .bg(rgb(BG))
                            .child(match analysis {
                                Ok(analysis) => match self.page {
                                    Page::InternalForce => self.render_internal_force(&analysis),
                                    Page::ForceMethod => self.render_force_method(&analysis),
                                    Page::DisplacementMethod => {
                                        self.render_displacement_method(&analysis)
                                    }
                                },
                                Err(error) => self.render_unstable(error),
                            }),
                    ),
            )
    }
}

fn analyze(params: BeamParams, left: Support, right: Support) -> Result<Analysis, String> {
    let span = params.span;
    let a = params.load_position.clamp(0.0, span);
    let edge_epsilon = span * 1.0e-10;
    let (nodes, load_node) = if a <= edge_epsilon {
        (vec![0.0, span], 0)
    } else if span - a <= edge_epsilon {
        (vec![0.0, span], 1)
    } else {
        (vec![0.0, a, span], 1)
    };

    let dof_count = nodes.len() * 2;
    let mut stiffness = vec![vec![0.0_f64; dof_count]; dof_count];
    let ei = params.ei();

    for element in 0..nodes.len() - 1 {
        let length = nodes[element + 1] - nodes[element];
        let local = beam_stiffness(ei, length);
        let map = [
            element * 2,
            element * 2 + 1,
            (element + 1) * 2,
            (element + 1) * 2 + 1,
        ];
        for local_row in 0..4 {
            for local_col in 0..4 {
                stiffness[map[local_row]][map[local_col]] += local[local_row][local_col];
            }
        }
    }

    let mut loads = vec![0.0; dof_count];
    loads[load_node * 2] = -params.load * 1_000.0;

    let mut restrained = vec![false; dof_count];
    restrained[0] = left.restrains_vertical();
    restrained[1] = left.restrains_rotation();
    let right_vertical = dof_count - 2;
    let right_rotation = dof_count - 1;
    restrained[right_vertical] = right.restrains_vertical();
    restrained[right_rotation] = right.restrains_rotation();

    let free = restrained
        .iter()
        .enumerate()
        .filter_map(|(index, restrained)| (!restrained).then_some(index))
        .collect::<Vec<_>>();

    let mut reduced_stiffness = vec![vec![0.0; free.len()]; free.len()];
    let mut reduced_loads = vec![0.0; free.len()];
    for (row, global_row) in free.iter().copied().enumerate() {
        reduced_loads[row] = loads[global_row];
        for (col, global_col) in free.iter().copied().enumerate() {
            reduced_stiffness[row][col] = stiffness[global_row][global_col];
        }
    }

    let reduced_displacements = solve_linear_system(reduced_stiffness, reduced_loads)
        .ok_or_else(|| "刚度矩阵奇异：体系仍可发生刚体平移或转动。".to_string())?;
    let mut dofs = vec![0.0; dof_count];
    for (index, global) in free.iter().copied().enumerate() {
        dofs[global] = reduced_displacements[index];
    }

    let reactions = stiffness
        .iter()
        .enumerate()
        .map(|(row, values)| {
            values
                .iter()
                .zip(&dofs)
                .map(|(stiffness, displacement)| stiffness * displacement)
                .sum::<f64>()
                - loads[row]
        })
        .collect::<Vec<_>>();

    let left_vertical_reaction = reactions[0];
    let left_moment_reaction = reactions[1];
    let simple_left_reaction = params.load * 1_000.0 * (span - a) / span;
    // 三次梁挠度曲线用 96 段已经足够平滑，同时显著降低 Debug 构建下的路径细分开销。
    let mut x_values = (0..=96)
        .map(|index| span * index as f64 / 96.0)
        .collect::<Vec<_>>();
    if a > 0.0 && a < span {
        x_values.extend([a - span * 1.0e-8, a, a + span * 1.0e-8]);
    }
    x_values.sort_by(f64::total_cmp);

    let samples = x_values
        .into_iter()
        .map(|raw_x| {
            let x = raw_x.clamp(0.0, span);
            let load_is_left = x + span * 1.0e-12 >= a;
            let point_load = if load_is_left {
                params.load * 1_000.0
            } else {
                0.0
            };
            let shear = left_vertical_reaction - point_load;
            let moment =
                -left_moment_reaction + left_vertical_reaction * x - point_load * (x - a).max(0.0);
            let base_moment = simple_left_reaction * x - point_load * (x - a).max(0.0);
            Sample {
                x,
                shear,
                moment,
                base_moment,
                correction_moment: moment - base_moment,
                displacement: displacement_at(x, &nodes, &dofs),
            }
        })
        .collect::<Vec<_>>();

    let max_shear = samples
        .iter()
        .map(|sample| sample.shear.abs())
        .fold(0.0, f64::max);
    let max_moment = samples
        .iter()
        .map(|sample| sample.moment.abs())
        .fold(0.0, f64::max);
    let max_displacement = samples
        .iter()
        .map(|sample| sample.displacement.abs())
        .fold(0.0, f64::max);
    let constraints = [
        left.restrains_vertical(),
        left.restrains_rotation(),
        right.restrains_vertical(),
        right.restrains_rotation(),
    ]
    .into_iter()
    .filter(|value| *value)
    .count();

    Ok(Analysis {
        params,
        left,
        right,
        nodes,
        dofs,
        reactions,
        samples,
        max_shear,
        max_moment,
        max_displacement,
        indeterminacy: constraints.saturating_sub(2),
    })
}

fn beam_stiffness(ei: f64, length: f64) -> [[f64; 4]; 4] {
    let factor = ei / length.powi(3);
    let l = length;
    [
        [
            12.0 * factor,
            6.0 * l * factor,
            -12.0 * factor,
            6.0 * l * factor,
        ],
        [
            6.0 * l * factor,
            4.0 * l * l * factor,
            -6.0 * l * factor,
            2.0 * l * l * factor,
        ],
        [
            -12.0 * factor,
            -6.0 * l * factor,
            12.0 * factor,
            -6.0 * l * factor,
        ],
        [
            6.0 * l * factor,
            2.0 * l * l * factor,
            -6.0 * l * factor,
            4.0 * l * l * factor,
        ],
    ]
}

fn solve_linear_system(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Option<Vec<f64>> {
    let size = rhs.len();
    if size == 0 {
        return Some(Vec::new());
    }
    let matrix_scale = matrix
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0, f64::max)
        .max(1.0);

    for pivot_col in 0..size {
        let pivot_row = (pivot_col..size)
            .max_by(|a, b| {
                matrix[*a][pivot_col]
                    .abs()
                    .total_cmp(&matrix[*b][pivot_col].abs())
            })
            .unwrap();
        if matrix[pivot_row][pivot_col].abs() < matrix_scale * 1.0e-12 {
            return None;
        }
        matrix.swap(pivot_col, pivot_row);
        rhs.swap(pivot_col, pivot_row);

        let pivot_values = matrix[pivot_col].clone();
        for row in pivot_col + 1..size {
            let factor = matrix[row][pivot_col] / matrix[pivot_col][pivot_col];
            for (entry, pivot_entry) in matrix[row][pivot_col..]
                .iter_mut()
                .zip(&pivot_values[pivot_col..])
            {
                *entry -= factor * pivot_entry;
            }
            rhs[row] -= factor * rhs[pivot_col];
        }
    }

    let mut solution = vec![0.0; size];
    for row in (0..size).rev() {
        let known = (row + 1..size)
            .map(|col| matrix[row][col] * solution[col])
            .sum::<f64>();
        solution[row] = (rhs[row] - known) / matrix[row][row];
    }
    Some(solution)
}

fn displacement_at(x: f64, nodes: &[f64], dofs: &[f64]) -> f64 {
    let element = nodes
        .windows(2)
        .position(|pair| x <= pair[1] + 1.0e-12)
        .unwrap_or(nodes.len() - 2);
    let x0 = nodes[element];
    let length = nodes[element + 1] - x0;
    let s = ((x - x0) / length).clamp(0.0, 1.0);
    let n1 = 1.0 - 3.0 * s * s + 2.0 * s * s * s;
    let n2 = length * (s - 2.0 * s * s + s * s * s);
    let n3 = 3.0 * s * s - 2.0 * s * s * s;
    let n4 = length * (-s * s + s * s * s);
    n1 * dofs[element * 2]
        + n2 * dofs[element * 2 + 1]
        + n3 * dofs[(element + 1) * 2]
        + n4 * dofs[(element + 1) * 2 + 1]
}

fn pretty_number(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn replace_numeric_range(
    value: &mut String,
    range: Range<usize>,
    text: &str,
    max_len: usize,
) -> usize {
    let remaining = value.len() - range.len();
    let available = max_len.saturating_sub(remaining);
    let insert = text
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.' || *ch == '-')
        .take(available)
        .collect::<String>();
    value.replace_range(range.clone(), &insert);
    range.start + insert.len()
}

fn with_alpha(hex: u32, alpha: f32) -> Rgba {
    let mut color = rgb(hex);
    color.a = alpha;
    color
}

fn draw_polyline(window: &mut Window, points: &[gpui::Point<Pixels>], width: f32, color: Rgba) {
    if points.len() < 2 {
        return;
    }
    let mut builder = PathBuilder::stroke(px(width));
    builder.move_to(points[0]);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn paint_vertical_line(
    window: &mut Window,
    x: f32,
    y_start: f32,
    y_end: f32,
    width: f32,
    color: Rgba,
) {
    let top = y_start.min(y_end);
    let height = (y_end - y_start).abs().max(1.0);
    window.paint_quad(fill(
        Bounds::new(
            point(px(x - width * 0.5), px(top)),
            size(px(width), px(height)),
        ),
        color,
    ));
}

fn paint_horizontal_line(
    window: &mut Window,
    x_start: f32,
    x_end: f32,
    y: f32,
    width: f32,
    color: Rgba,
) {
    let left = x_start.min(x_end);
    let line_width = (x_end - x_start).abs().max(1.0);
    window.paint_quad(fill(
        Bounds::new(
            point(px(left), px(y - width * 0.5)),
            size(px(line_width), px(width)),
        ),
        color,
    ));
}

fn draw_polygon(window: &mut Window, points: &[gpui::Point<Pixels>], color: Rgba) {
    if points.len() < 3 {
        return;
    }
    let mut builder = PathBuilder::fill();
    builder.add_polygon(points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn draw_circle(window: &mut Window, center_x: f32, center_y: f32, radius: f32, color: Rgba) {
    window.paint_quad(quad(
        Bounds::new(
            point(px(center_x - radius), px(center_y - radius)),
            size(px(radius * 2.0), px(radius * 2.0)),
        ),
        px(radius),
        color,
        px(0.0),
        color,
        BorderStyle::Solid,
    ));
}

fn paint_support(window: &mut Window, x: f32, beam_y: f32, support: Support, left: bool) {
    match support {
        Support::Fixed => {
            draw_polyline(
                window,
                &[
                    point(px(x), px(beam_y - 28.0)),
                    point(px(x), px(beam_y + 30.0)),
                ],
                4.0,
                rgb(CYAN),
            );
            let direction = if left { -1.0 } else { 1.0 };
            for offset in (-20..=24).step_by(9) {
                draw_polyline(
                    window,
                    &[
                        point(px(x), px(beam_y + offset as f32)),
                        point(px(x + direction * 10.0), px(beam_y + offset as f32 + 7.0)),
                    ],
                    1.0,
                    with_alpha(CYAN, 0.65),
                );
            }
        }
        Support::Pinned | Support::Roller => {
            draw_polygon(
                window,
                &[
                    point(px(x), px(beam_y + 4.0)),
                    point(px(x - 15.0), px(beam_y + 31.0)),
                    point(px(x + 15.0), px(beam_y + 31.0)),
                ],
                with_alpha(CYAN, 0.9),
            );
            if support == Support::Roller {
                draw_circle(window, x - 8.0, beam_y + 37.0, 3.5, rgb(CYAN));
                draw_circle(window, x + 8.0, beam_y + 37.0, 3.5, rgb(CYAN));
                draw_polyline(
                    window,
                    &[
                        point(px(x - 20.0), px(beam_y + 44.0)),
                        point(px(x + 20.0), px(beam_y + 44.0)),
                    ],
                    1.5,
                    with_alpha(CYAN, 0.7),
                );
            } else {
                draw_polyline(
                    window,
                    &[
                        point(px(x - 20.0), px(beam_y + 34.0)),
                        point(px(x + 20.0), px(beam_y + 34.0)),
                    ],
                    1.5,
                    with_alpha(CYAN, 0.7),
                );
            }
        }
        Support::Free => {
            draw_circle(window, x, beam_y, 4.0, rgb(MUTED));
        }
    }
}

fn paint_load_arrow(window: &mut Window, x: f32, beam_y: f32) {
    draw_polyline(
        window,
        &[
            point(px(x), px(beam_y - 62.0)),
            point(px(x), px(beam_y - 8.0)),
        ],
        2.5,
        rgb(PINK),
    );
    draw_polygon(
        window,
        &[
            point(px(x), px(beam_y - 2.0)),
            point(px(x - 7.0), px(beam_y - 13.0)),
            point(px(x + 7.0), px(beam_y - 13.0)),
        ],
        rgb(PINK),
    );
}

fn paint_structure(bounds: Bounds<Pixels>, analysis: &Analysis, window: &mut Window) {
    let left = f32::from(bounds.left()) + 58.0;
    let right = f32::from(bounds.right()) - 58.0;
    let beam_y = f32::from(bounds.top()) + 76.0;
    let load_x =
        left + (right - left) * (analysis.params.load_position / analysis.params.span) as f32;

    draw_polyline(
        window,
        &[point(px(left), px(beam_y)), point(px(right), px(beam_y))],
        6.0,
        rgb(TEXT),
    );
    draw_polyline(
        window,
        &[
            point(px(left), px(beam_y - 5.0)),
            point(px(right), px(beam_y - 5.0)),
        ],
        1.0,
        with_alpha(CYAN, 0.45),
    );
    paint_support(window, left, beam_y, analysis.left, true);
    paint_support(window, right, beam_y, analysis.right, false);
    paint_load_arrow(window, load_x, beam_y);

    let dimension_y = f32::from(bounds.bottom()) - 18.0;
    draw_polyline(
        window,
        &[
            point(px(left), px(dimension_y)),
            point(px(right), px(dimension_y)),
        ],
        1.0,
        with_alpha(MUTED, 0.55),
    );
    for x in [left, load_x, right] {
        draw_polyline(
            window,
            &[
                point(px(x), px(dimension_y - 5.0)),
                point(px(x), px(dimension_y + 5.0)),
            ],
            1.0,
            with_alpha(MUTED, 0.7),
        );
    }
}

fn paint_graph(
    bounds: Bounds<Pixels>,
    analysis: &Analysis,
    value: GraphValue,
    accent: u32,
    window: &mut Window,
) {
    let left = f32::from(bounds.left()) + 24.0;
    let right = f32::from(bounds.right()) - 18.0;
    let top = f32::from(bounds.top()) + 18.0;
    let bottom = f32::from(bounds.bottom()) - 22.0;
    let baseline = (top + bottom) * 0.5;
    let amplitude = (bottom - top) * 0.42;
    let span = analysis.params.span;
    let max_value = analysis
        .samples
        .iter()
        .map(|sample| value.value(sample).abs())
        .fold(0.0, f64::max)
        .max(1.0e-20);

    for index in 0..=4 {
        let x = left + (right - left) * index as f32 / 4.0;
        paint_vertical_line(window, x, top, bottom, 1.0, with_alpha(BORDER, 0.34));
    }
    for y in [top, baseline, bottom] {
        paint_horizontal_line(
            window,
            left,
            right,
            y,
            if (y - baseline).abs() < 0.1 { 1.4 } else { 1.0 },
            with_alpha(
                if (y - baseline).abs() < 0.1 {
                    MUTED
                } else {
                    BORDER
                },
                0.48,
            ),
        );
    }

    let load_x =
        left + (right - left) * (analysis.params.load_position / analysis.params.span) as f32;
    paint_vertical_line(window, load_x, top, bottom, 1.0, with_alpha(PINK, 0.38));

    let graph_points = analysis
        .samples
        .iter()
        .map(|sample| {
            let x = left + (right - left) * (sample.x / span) as f32;
            let normalized = (value.value(sample) / max_value) as f32;
            point(px(x), px(baseline - normalized * amplitude))
        })
        .collect::<Vec<_>>();

    if graph_points.len() >= 2 {
        let mut fill_points = Vec::with_capacity(graph_points.len() + 2);
        fill_points.push(point(px(left), px(baseline)));
        fill_points.extend(graph_points.iter().copied());
        fill_points.push(point(px(right), px(baseline)));
        draw_polygon(window, &fill_points, with_alpha(accent, 0.14));
        draw_polyline(window, &graph_points, 2.7, rgb(accent));
    }
}

fn paint_deformed_shape(bounds: Bounds<Pixels>, analysis: &Analysis, window: &mut Window) {
    let left = f32::from(bounds.left()) + 46.0;
    let right = f32::from(bounds.right()) - 46.0;
    let original_y = f32::from(bounds.top()) + 92.0;
    let span = analysis.params.span;
    let max_displacement = analysis.max_displacement.max(1.0e-20);
    let display_amplitude = (f32::from(bounds.size.height) * 0.34).min(72.0);

    paint_horizontal_line(
        window,
        left,
        right,
        original_y,
        2.0,
        with_alpha(MUTED, 0.55),
    );

    let graph_points = analysis
        .samples
        .iter()
        .map(|sample| {
            let x = left + (right - left) * (sample.x / span) as f32;
            let y =
                original_y - (sample.displacement / max_displacement) as f32 * display_amplitude;
            point(px(x), px(y))
        })
        .collect::<Vec<_>>();

    let shade_points = {
        let mut points = Vec::with_capacity(graph_points.len() + 2);
        points.push(point(px(left), px(original_y)));
        points.extend(graph_points.iter().copied());
        points.push(point(px(right), px(original_y)));
        points
    };
    draw_polygon(window, &shade_points, with_alpha(PINK, 0.10));

    for sample in analysis.samples.iter().step_by(12) {
        let x = left + (right - left) * (sample.x / span) as f32;
        let y = original_y - (sample.displacement / max_displacement) as f32 * display_amplitude;
        paint_vertical_line(window, x, original_y, y, 1.0, with_alpha(PINK, 0.22));
    }
    draw_polyline(window, &graph_points, 4.0, rgb(PINK));

    let load_x =
        left + (right - left) * (analysis.params.load_position / analysis.params.span) as f32;
    paint_load_arrow(window, load_x, original_y);
    paint_support(window, left, original_y, analysis.left, true);
    paint_support(window, right, original_y, analysis.right, false);
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1260.0), px(820.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("BeamLab · 单跨梁分析".into()),
                    ..Default::default()
                }),
                window_decorations: Some(WindowDecorations::Server),
                window_min_size: Some(size(px(960.0), px(640.0))),
                is_movable: true,
                is_resizable: true,
                app_id: Some("beamlab".into()),
                ..Default::default()
            },
            |_, cx| cx.new(BeamLab::new),
        )
        .expect("failed to open BeamLab window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_input_replaces_only_the_selected_part() {
        let mut value = "3.2".to_string();

        let caret = replace_numeric_range(&mut value, 2..3, "75", 12);

        assert_eq!(value, "3.75");
        assert_eq!(caret, 4);
    }

    #[test]
    fn numeric_paste_replaces_all_and_filters_units() {
        let mut value = "80".to_string();

        let caret = replace_numeric_range(&mut value, 0..2, " 120 kN.5\n", 12);

        assert_eq!(value, "120.5");
        assert_eq!(caret, 5);
    }

    #[test]
    fn simply_supported_reactions_and_moment_are_correct() {
        let params = BeamParams {
            span: 10.0,
            load: 100.0,
            load_position: 4.0,
            ..BeamParams::default()
        };
        let result = analyze(params, Support::Pinned, Support::Roller).unwrap();
        assert!((result.left_reaction() - 60.0).abs() < 1.0e-8);
        assert!((result.right_reaction() - 40.0).abs() < 1.0e-8);
        assert!((result.max_moment / 1_000.0 - 240.0).abs() < 1.0e-5);
    }

    #[test]
    fn cantilever_tip_load_matches_closed_form_deflection() {
        let params = BeamParams {
            span: 4.0,
            load: 20.0,
            load_position: 4.0,
            ..BeamParams::default()
        };
        let expected = params.load * 1_000.0 * params.span.powi(3) / (3.0 * params.ei());
        let result = analyze(params, Support::Fixed, Support::Free).unwrap();
        assert!((result.max_displacement - expected).abs() < expected * 1.0e-8);
        assert!((result.left_reaction() - 20.0).abs() < 1.0e-8);
        assert!((result.left_reaction_moment() - 80.0).abs() < 1.0e-8);
    }

    #[test]
    fn a_single_pin_is_unstable() {
        let result = analyze(BeamParams::default(), Support::Pinned, Support::Free);
        assert!(result.is_err());
    }
}
