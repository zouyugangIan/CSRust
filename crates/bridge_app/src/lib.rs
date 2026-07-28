//! BridgeLab GPUI application.
//!
//! 运行：
//! cargo run -p bridge-app

use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use bridge_core::{BridgeModel, ModelParts};
use bridge_io::{ensure_project_extension, load_project, save_project};
use bridge_solver::{AnalysisResult as SolverAnalysis, SolveOptions, solve};
use bridge_validation::{ValidationReport, validate_result};
use gpui::{
    AnyView, App, AppContext, Application, BorderStyle, Bounds, ClipboardItem, Context,
    CursorStyle, Div, Element, ElementId, Entity, FocusHandle, FontWeight, GlobalElementId,
    InspectorElementId, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, PathBuilder, PathPromptOptions, Pixels, PromptButton, PromptLevel,
    Render, Rgba, ShapedLine, Style, StyleRefinement, TextRun, Timer, TitlebarOptions, Window,
    WindowBounds, WindowControlArea, WindowDecorations, WindowOptions, canvas, div, fill, point,
    prelude::*, px, quad, relative, rgb, size,
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

#[cfg(target_os = "macos")]
const SHORTCUT_NEW: &str = "⌘N";
#[cfg(not(target_os = "macos"))]
const SHORTCUT_NEW: &str = "Ctrl+N";
#[cfg(target_os = "macos")]
const SHORTCUT_OPEN: &str = "⌘O";
#[cfg(not(target_os = "macos"))]
const SHORTCUT_OPEN: &str = "Ctrl+O";
#[cfg(target_os = "macos")]
const SHORTCUT_SAVE: &str = "⌘S";
#[cfg(not(target_os = "macos"))]
const SHORTCUT_SAVE: &str = "Ctrl+S";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    InternalForce,
    ForceMethod,
    DisplacementMethod,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolbarAction {
    New,
    Open,
    Save,
    Undo,
    Redo,
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
    project_name: String,
    spans: Vec<f64>,
    load: f64,
    load_position: f64,
    elastic_modulus: f64,
    inertia_millionth: f64,
    area_m2: f64,
    density_kg_m3: f64,
    material_name: String,
    section_name: String,
    load_case_name: String,
}

impl Default for BeamParams {
    fn default() -> Self {
        Self {
            project_name: "BridgeLab 三跨桥".to_string(),
            spans: vec![8.0, 10.0, 8.0],
            load: 80.0,
            load_position: 12.0,
            elastic_modulus: 200.0,
            inertia_millionth: 8.0,
            area_m2: 0.12,
            density_kg_m3: 7_850.0,
            material_name: "主梁材料".to_string(),
            section_name: "主梁截面".to_string(),
            load_case_name: "LC1 恒载".to_string(),
        }
    }
}

impl BeamParams {
    fn field_values(&self) -> [String; FIELD_COUNT] {
        [
            self.span_layout(),
            pretty_number(self.load),
            pretty_number(self.load_position),
            pretty_number(self.elastic_modulus),
            pretty_number(self.inertia_millionth),
        ]
    }

    fn ei(&self) -> f64 {
        self.elastic_modulus * 1.0e9 * self.inertia_millionth * 1.0e-6
    }

    fn total_span(&self) -> f64 {
        self.spans.iter().sum()
    }

    fn span_layout(&self) -> String {
        self.spans
            .iter()
            .map(|span| pretty_number(*span))
            .collect::<Vec<_>>()
            .join(", ")
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
    model: BridgeModel,
    solver: SolverAnalysis,
    validation: ValidationReport,
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

#[derive(Clone, Debug, PartialEq)]
struct EditorSnapshot {
    params: BeamParams,
    left_support: Support,
    right_support: Support,
}

#[derive(Clone, Debug)]
struct SaveCoordinator {
    generation: Arc<AtomicU64>,
    lock: Arc<Mutex<()>>,
}

impl Default for SaveCoordinator {
    fn default() -> Self {
        Self {
            generation: Arc::new(AtomicU64::new(0)),
            lock: Arc::new(Mutex::new(())),
        }
    }
}

impl SaveCoordinator {
    fn begin(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SaveTaskOutcome {
    Saved,
    Superseded,
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
    project_path: Option<PathBuf>,
    saved_snapshot: EditorSnapshot,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    edit_origin: Option<EditorSnapshot>,
    editing_field: Option<usize>,
    analysis_revision: u64,
    is_solving: bool,
    status_message: String,
    needs_migration_save: bool,
    save_coordinator: SaveCoordinator,
    document_revision: u64,
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
        let focused = app.field_focus[self.index].is_focused(window);
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
        let (selection, cursor) = if !focused {
            (None, None)
        } else if start == end {
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
        let saved_snapshot = EditorSnapshot {
            params: params.clone(),
            left_support,
            right_support,
        };

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
            project_path: None,
            saved_snapshot,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            edit_origin: None,
            editing_field: None,
            analysis_revision: 0,
            is_solving: false,
            status_message: "新工程 · 尚未保存".to_string(),
            needs_migration_save: false,
            save_coordinator: SaveCoordinator::default(),
            document_revision: 0,
        }
    }

    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            params: self.params.clone(),
            left_support: self.left_support,
            right_support: self.right_support,
        }
    }

    fn matches_snapshot(&self, snapshot: &EditorSnapshot) -> bool {
        self.params == snapshot.params
            && self.left_support == snapshot.left_support
            && self.right_support == snapshot.right_support
    }

    fn is_dirty(&self) -> bool {
        self.needs_migration_save
            || self.input_error.is_some()
            || !self.matches_snapshot(&self.saved_snapshot)
    }

    fn begin_field_edit(&mut self, index: usize) {
        if self.editing_field == Some(index) {
            return;
        }
        self.commit_edit_group();
        self.edit_origin = Some(self.snapshot());
        self.editing_field = Some(index);
    }

    fn commit_edit_group(&mut self) {
        let Some(origin) = self.edit_origin.take() else {
            self.editing_field = None;
            return;
        };
        if !self.matches_snapshot(&origin) {
            self.push_undo(origin);
        }
        self.editing_field = None;
    }

    fn cancel_edit_group(&mut self, cx: &mut Context<Self>) {
        if let Some(origin) = self.edit_origin.take() {
            self.editing_field = None;
            self.restore_snapshot(origin, cx);
        } else {
            self.restore_fields();
        }
    }

    fn push_undo(&mut self, snapshot: EditorSnapshot) {
        if self.undo_stack.last() != Some(&snapshot) {
            self.undo_stack.push(snapshot);
            if self.undo_stack.len() > 100 {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
    }

    fn restore_snapshot(&mut self, snapshot: EditorSnapshot, cx: &mut Context<Self>) {
        self.params = snapshot.params;
        self.left_support = snapshot.left_support;
        self.right_support = snapshot.right_support;
        self.restore_fields();
        self.refresh_analysis(cx);
        self.schedule_autosave(cx);
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        self.commit_edit_group();
        let Some(previous) = self.undo_stack.pop() else {
            self.status_message = "没有可撤销的操作".to_string();
            cx.notify();
            return;
        };
        self.redo_stack.push(self.snapshot());
        self.restore_snapshot(previous, cx);
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        self.commit_edit_group();
        let Some(next) = self.redo_stack.pop() else {
            self.status_message = "没有可重做的操作".to_string();
            cx.notify();
            return;
        };
        self.undo_stack.push(self.snapshot());
        self.restore_snapshot(next, cx);
    }

    fn record_discrete_change(&mut self, origin: EditorSnapshot, cx: &mut Context<Self>) {
        if !self.matches_snapshot(&origin) {
            self.push_undo(origin);
            self.refresh_analysis(cx);
            self.schedule_autosave(cx);
        }
    }

    fn invalidate_document_tasks(&mut self) {
        self.document_revision = self.document_revision.wrapping_add(1);
        self.save_coordinator.begin();
    }

    fn schedule_autosave(&self, cx: &mut Context<Self>) {
        let Some(path) = self.project_path.clone() else {
            return;
        };
        let document_revision = self.document_revision;
        let snapshot = self.snapshot();
        let Ok(model) = build_model(
            &snapshot.params,
            snapshot.left_support,
            snapshot.right_support,
        ) else {
            return;
        };
        let save_coordinator = self.save_coordinator.clone();
        cx.spawn(async move |this, cx| {
            Timer::after(Duration::from_millis(900)).await;
            let should_save = this
                .read_with(cx, |app, _| {
                    app.document_revision == document_revision
                        && app.project_path.as_deref() == Some(path.as_path())
                        && app.matches_snapshot(&snapshot)
                        && app.input_error.is_none()
                        && app.is_dirty()
                })
                .unwrap_or(false);
            if !should_save {
                return;
            }
            let worker_path = path.clone();
            let generation = save_coordinator.begin();
            let worker_coordinator = save_coordinator.clone();
            let task = cx.background_spawn(async move {
                coordinated_save(&worker_coordinator, generation, &worker_path, &model)
            });
            let result = task.await;
            let _ = this.update(cx, move |app, cx| {
                if app.document_revision != document_revision
                    || app.project_path.as_deref() != Some(path.as_path())
                    || !save_coordinator.is_current(generation)
                    || !app.matches_snapshot(&snapshot)
                {
                    return;
                }
                match result {
                    Ok(SaveTaskOutcome::Saved) => {
                        app.saved_snapshot = snapshot;
                        app.needs_migration_save = false;
                        app.status_message = "已自动保存".to_string();
                    }
                    Ok(SaveTaskOutcome::Superseded) => return,
                    Err(error) => {
                        app.status_message = format!("自动保存失败：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.commit_edit_group();
        if self.input_error.is_some() {
            self.status_message = "请先修正输入错误再保存".to_string();
            cx.notify();
            return;
        }
        if let Some(path) = self.project_path.clone() {
            self.save_to_path(path, cx);
        } else {
            self.save_as(cx);
        }
    }

    fn save_as(&mut self, cx: &mut Context<Self>) {
        self.commit_edit_group();
        if self.input_error.is_some() {
            self.status_message = "请先修正输入错误再保存".to_string();
            cx.notify();
            return;
        }
        let default_name = format!("{}.bridge.json", safe_file_stem(&self.params.project_name));
        let directory = self
            .project_path
            .as_deref()
            .and_then(Path::parent)
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let document_revision = self.document_revision;
        let selection = cx.prompt_for_new_path(directory, Some(&default_name));
        cx.spawn(async move |this, cx| {
            let path = match selection.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = this.update(cx, move |app, cx| {
                        if app.document_revision != document_revision {
                            return;
                        }
                        app.status_message = format!("保存对话框失败：{error}");
                        cx.notify();
                    });
                    return;
                }
                Err(_) => {
                    let _ = this.update(cx, move |app, cx| {
                        if app.document_revision != document_revision {
                            return;
                        }
                        app.status_message = "保存对话框已意外关闭".to_string();
                        cx.notify();
                    });
                    return;
                }
            };
            let path = ensure_project_extension(path);
            let _ = this.update(cx, |app, cx| {
                if app.document_revision == document_revision {
                    app.save_to_path(path, cx);
                }
            });
        })
        .detach();
    }

    fn save_to_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.input_error.is_some() {
            self.status_message = "请先修正输入错误再保存".to_string();
            cx.notify();
            return;
        }
        let snapshot = self.snapshot();
        let model = match build_model(
            &snapshot.params,
            snapshot.left_support,
            snapshot.right_support,
        ) {
            Ok(model) => model,
            Err(error) => {
                self.status_message = format!("无法保存：{error}");
                cx.notify();
                return;
            }
        };
        let document_revision = self.document_revision;
        self.status_message = "正在保存…".to_string();
        let worker_path = path.clone();
        let save_coordinator = self.save_coordinator.clone();
        let generation = save_coordinator.begin();
        let worker_coordinator = save_coordinator.clone();
        let task = cx.background_executor().spawn(async move {
            coordinated_save(&worker_coordinator, generation, &worker_path, &model)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, move |app, cx| {
                if app.document_revision != document_revision
                    || !save_coordinator.is_current(generation)
                {
                    return;
                }
                match result {
                    Ok(SaveTaskOutcome::Saved) => {
                        app.project_path = Some(path.clone());
                        app.saved_snapshot = snapshot;
                        app.needs_migration_save = false;
                        app.status_message = format!("已保存 · {}", display_path(&path));
                        cx.add_recent_document(&path);
                    }
                    Ok(SaveTaskOutcome::Superseded) => return,
                    Err(error) => {
                        app.status_message = format!("保存失败：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_edit_group();
        if self.is_dirty() {
            let prompt = window.prompt(
                PromptLevel::Warning,
                "当前工程有未保存更改",
                Some("继续打开其他工程将放弃这些更改。"),
                &[PromptButton::ok("放弃并打开"), PromptButton::cancel("取消")],
                cx,
            );
            cx.spawn(async move |this, cx| {
                if prompt.await.unwrap_or(1) == 0 {
                    let _ = this.update(cx, |app, cx| app.open_dialog(cx));
                }
            })
            .detach();
        } else {
            self.open_dialog(cx);
        }
    }

    fn open_dialog(&mut self, cx: &mut Context<Self>) {
        let document_revision = self.document_revision;
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("打开 BridgeLab 工程".into()),
        });
        cx.spawn(async move |this, cx| {
            let path = match selection.await {
                Ok(Ok(Some(paths))) => {
                    let Some(path) = paths.into_iter().next() else {
                        return;
                    };
                    path
                }
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = this.update(cx, move |app, cx| {
                        if app.document_revision != document_revision {
                            return;
                        }
                        app.status_message = format!("打开对话框失败：{error}");
                        cx.notify();
                    });
                    return;
                }
                Err(_) => {
                    let _ = this.update(cx, move |app, cx| {
                        if app.document_revision != document_revision {
                            return;
                        }
                        app.status_message = "打开对话框已意外关闭".to_string();
                        cx.notify();
                    });
                    return;
                }
            };
            let worker_path = path.clone();
            let task = cx.background_spawn(async move { load_project(&worker_path) });
            let result = task.await;
            let _ = this.update(cx, move |app, cx| {
                if app.document_revision != document_revision {
                    return;
                }
                match result {
                    Ok(loaded) => {
                        if let Err(error) =
                            app.install_loaded_project(loaded.model, path, loaded.migrated_from, cx)
                        {
                            app.status_message = format!("打开失败：{error}");
                            cx.notify();
                        }
                    }
                    Err(error) => {
                        app.status_message = format!("打开失败：{error}");
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn install_loaded_project(
        &mut self,
        model: BridgeModel,
        path: PathBuf,
        migrated_from: Option<u32>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (params, left_support, right_support) = editor_state_from_model(&model)?;
        self.invalidate_document_tasks();
        self.params = params;
        self.left_support = left_support;
        self.right_support = right_support;
        self.project_path = Some(path.clone());
        self.restore_fields();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.edit_origin = None;
        self.editing_field = None;
        self.saved_snapshot = self.snapshot();
        self.needs_migration_save = migrated_from.is_some();
        self.status_message = if let Some(version) = migrated_from {
            format!("已从 v{version} 迁移 · 请保存升级")
        } else {
            format!("已打开 · {}", display_path(&path))
        };
        cx.add_recent_document(&path);
        self.refresh_analysis(cx);
        Ok(())
    }

    fn request_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_edit_group();
        if self.is_dirty() {
            let prompt = window.prompt(
                PromptLevel::Warning,
                "当前工程有未保存更改",
                Some("新建工程将放弃这些更改。"),
                &[PromptButton::ok("放弃并新建"), PromptButton::cancel("取消")],
                cx,
            );
            cx.spawn(async move |this, cx| {
                if prompt.await.unwrap_or(1) == 0 {
                    let _ = this.update(cx, |app, cx| app.new_project(cx));
                }
            })
            .detach();
        } else {
            self.new_project(cx);
        }
    }

    fn new_project(&mut self, cx: &mut Context<Self>) {
        self.invalidate_document_tasks();
        self.params = BeamParams::default();
        self.left_support = Support::Pinned;
        self.right_support = Support::Roller;
        self.project_path = None;
        self.restore_fields();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.edit_origin = None;
        self.editing_field = None;
        self.saved_snapshot = self.snapshot();
        self.needs_migration_save = false;
        self.status_message = "新工程 · 尚未保存".to_string();
        self.refresh_analysis(cx);
    }

    fn refresh_analysis(&mut self, cx: &mut Context<Self>) {
        self.analysis_revision = self.analysis_revision.wrapping_add(1);
        let revision = self.analysis_revision;
        let params = self.params.clone();
        let left_support = self.left_support;
        let right_support = self.right_support;
        self.is_solving = true;
        self.status_message = "正在分析…".to_string();

        cx.spawn(async move |this, cx| {
            Timer::after(Duration::from_millis(80)).await;
            let is_current = this
                .read_with(cx, |app, _| app.analysis_revision == revision)
                .unwrap_or(false);
            if !is_current {
                return;
            }
            let task =
                cx.background_spawn(async move { analyze(params, left_support, right_support) });
            let result = task.await;
            let _ = this.update(cx, move |app, cx| {
                if app.analysis_revision != revision {
                    return;
                }
                app.is_solving = false;
                match result {
                    Ok(analysis) => {
                        let analysis = Arc::new(analysis);
                        app.status_message = analysis.validation.summary();
                        app.analysis = Ok(Arc::clone(&analysis));
                        for canvas in &app.canvases {
                            let analysis = Arc::clone(&analysis);
                            canvas.update(cx, move |canvas, cx| {
                                canvas.analysis = analysis;
                                cx.notify();
                            });
                        }
                    }
                    Err(error) => {
                        app.status_message = "分析失败".to_string();
                        app.analysis = Err(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_fields(&mut self, cx: &mut Context<Self>) {
        let spans = match parse_spans(&self.fields[0]) {
            Ok(spans) => spans,
            Err(error) => {
                self.input_error = Some(error);
                return;
            }
        };
        let values = match self.fields[1..]
            .iter()
            .map(|value| value.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(values) => values,
            Err(_) => {
                self.input_error = Some("请输入有效数字".into());
                return;
            }
        };

        let candidate = BeamParams {
            project_name: self.params.project_name.clone(),
            spans,
            load: values[0],
            load_position: values[1],
            elastic_modulus: values[2],
            inertia_millionth: values[3],
            area_m2: self.params.area_m2,
            density_kg_m3: self.params.density_kg_m3,
            material_name: self.params.material_name.clone(),
            section_name: self.params.section_name.clone(),
            load_case_name: self.params.load_case_name.clone(),
        };

        let total_span = candidate.total_span();
        let error = if candidate.spans.len() > 20 {
            Some("当前版本最多支持 20 跨")
        } else if candidate
            .spans
            .iter()
            .any(|span| !(0.5..=200.0).contains(span))
        {
            Some("每个跨径应在 0.5～200 m")
        } else if total_span > 2_000.0 {
            Some("桥梁总长不应超过 2000 m")
        } else if !(0.01..=100_000.0).contains(&candidate.load) {
            Some("集中力 P 应大于 0")
        } else if !(0.0..=total_span).contains(&candidate.load_position) {
            Some("力位置 x 应位于桥梁范围内")
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
                self.schedule_autosave(cx);
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
        self.begin_field_edit(index);
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
        let caret = replace_field_range(&mut self.fields[index], range, text, index == 0);
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
        let mutates_field = if command {
            matches!(key, "x" | "v")
        } else {
            matches!(key, "backspace" | "delete" | "up" | "down")
                || event.keystroke.key_char.as_deref().is_some_and(|text| {
                    text.chars()
                        .all(|character| is_field_character(character, index == 0))
                })
        };
        if mutates_field {
            self.begin_field_edit(index);
        }
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
                if self.input_error.is_some() {
                    cx.notify();
                    return;
                }
                self.commit_edit_group();
                window.focus(&self.root_focus);
                cx.notify();
                return;
            }
            "escape" => {
                self.cancel_edit_group(cx);
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
                    && text.chars().all(|ch| is_field_character(ch, index == 0))
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
        if index == 0 {
            let mut spans =
                parse_spans(&self.fields[0]).unwrap_or_else(|_| self.params.spans.clone());
            for span in &mut spans {
                *span = (*span + STEPS[0] * direction).clamp(0.5, 200.0);
            }
            self.fields[0] = spans
                .iter()
                .map(|span| pretty_number(*span))
                .collect::<Vec<_>>()
                .join(", ");
            self.select_all_field(0);
            self.apply_fields(cx);
            return;
        }
        let fallback = match index {
            1 => self.params.load,
            2 => self.params.load_position,
            3 => self.params.elastic_modulus,
            _ => self.params.inertia_millionth,
        };
        let current = self.fields[index].parse::<f64>().unwrap_or(fallback);
        let mut next = current + STEPS[index] * direction;

        next = match index {
            1 => next.max(0.01),
            2 => next.clamp(0.0, self.params.total_span()),
            3 | 4 => next.max(STEPS[index]),
            _ => next,
        };

        self.fields[index] = pretty_number(next);
        self.select_all_field(index);
        self.apply_fields(cx);
    }

    fn set_preset(&mut self, preset: usize, cx: &mut Context<Self>) {
        self.commit_edit_group();
        let origin = self.snapshot();
        let supports = match preset {
            0 => (Support::Pinned, Support::Roller),
            1 => (Support::Fixed, Support::Free),
            _ => (Support::Fixed, Support::Fixed),
        };
        if (self.left_support, self.right_support) != supports {
            (self.left_support, self.right_support) = supports;
            self.record_discrete_change(origin, cx);
            cx.notify();
        }
    }

    fn on_global_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let command = modifiers.control || modifiers.platform;
        if !command {
            return;
        }
        match event.keystroke.key.as_str() {
            "s" if modifiers.shift => self.save_as(cx),
            "s" => self.save(cx),
            "o" => self.request_open(window, cx),
            "n" => self.request_new(window, cx),
            "z" if modifiers.shift => self.redo(cx),
            "z" => self.undo(cx),
            "y" => self.redo(cx),
            _ => return,
        }
        cx.notify();
    }

    fn render_toolbar_button(
        &self,
        action: ToolbarAction,
        label: &'static str,
        shortcut: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("toolbar", action as usize))
            .flex()
            .items_center()
            .gap_1()
            .h_9()
            .px_3()
            .rounded_lg()
            .cursor(if enabled {
                CursorStyle::PointingHand
            } else {
                CursorStyle::Arrow
            })
            .bg(rgb(PANEL))
            .border_1()
            .border_color(rgb(BORDER))
            .text_xs()
            .text_color(rgb(if enabled { TEXT } else { 0x50687a }))
            .when(enabled, |button| {
                button
                    .hover(|style| style.border_color(rgb(CYAN)).bg(rgb(CARD)))
                    .on_click(cx.listener(move |app, _, window, cx| match action {
                        ToolbarAction::New => app.request_new(window, cx),
                        ToolbarAction::Open => app.request_open(window, cx),
                        ToolbarAction::Save => app.save(cx),
                        ToolbarAction::Undo => app.undo(cx),
                        ToolbarAction::Redo => app.redo(cx),
                    }))
            })
            .child(label)
            .child(
                div()
                    .text_color(rgb(0x607b8f))
                    .text_size(px(10.0))
                    .child(shortcut),
            )
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
                                this.begin_field_edit(index);
                                this.nudge_field(index, -1.0, cx);
                                this.commit_edit_group();
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
                                this.begin_field_edit(index);
                                this.nudge_field(index, 1.0, cx);
                                this.commit_edit_group();
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
                        this.commit_edit_group();
                        let origin = this.snapshot();
                        if is_left {
                            this.left_support = this.left_support.next();
                        } else {
                            this.right_support = this.right_support.next();
                        }
                        this.record_discrete_change(origin, cx);
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
                                    .child("拖选/双击全选 · 跨径用逗号分隔"),
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
            .child(self.render_field(0, "跨径布置", "Lᵢ", "m", window, cx))
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
                        "模型：多跨等截面 Euler–Bernoulli 梁；中间节点默认滚动支承，忽略剪切与轴向变形。",
                    ),
            )
    }

    fn render_header(&self, stable: bool, cx: &mut Context<Self>) -> Div {
        let operation_failed = self.status_message.contains("失败")
            || self.status_message.starts_with("无法")
            || self.status_message.starts_with("请先");
        let status_color = if operation_failed {
            RED
        } else if self.needs_migration_save {
            AMBER
        } else if self.is_solving {
            BLUE
        } else if !stable {
            RED
        } else if self.is_dirty() {
            AMBER
        } else {
            GREEN
        };
        let status_text = if operation_failed {
            self.status_message.clone()
        } else if self.needs_migration_save {
            "旧版工程待保存升级".to_string()
        } else if self.is_solving {
            "正在后台分析".to_string()
        } else if !stable {
            "模型不稳定".to_string()
        } else if self.is_dirty() {
            "有未保存更改".to_string()
        } else {
            self.status_message.clone()
        };
        let can_undo = !self.undo_stack.is_empty()
            || self
                .edit_origin
                .as_ref()
                .is_some_and(|origin| !self.matches_snapshot(origin));
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
                                    .child("BridgeLab"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(self.params.project_name.clone()),
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
                    .gap_1()
                    .child(self.render_toolbar_button(
                        ToolbarAction::New,
                        "新建",
                        SHORTCUT_NEW,
                        true,
                        cx,
                    ))
                    .child(self.render_toolbar_button(
                        ToolbarAction::Open,
                        "打开",
                        SHORTCUT_OPEN,
                        true,
                        cx,
                    ))
                    .child(self.render_toolbar_button(
                        ToolbarAction::Save,
                        "保存",
                        SHORTCUT_SAVE,
                        true,
                        cx,
                    ))
                    .child(self.render_toolbar_button(ToolbarAction::Undo, "↶", "", can_undo, cx))
                    .child(self.render_toolbar_button(
                        ToolbarAction::Redo,
                        "↷",
                        "",
                        !self.redo_stack.is_empty(),
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_full()
                    .max_w(px(180.0))
                    .overflow_hidden()
                    .bg(with_alpha(status_color, 0.16))
                    .text_xs()
                    .text_color(rgb(status_color))
                    .child(div().size_2().rounded_full().bg(rgb(status_color)))
                    .child(status_text),
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
            "{} 跨 / L = {} m   ·   P = {} kN @ {} m",
            analysis.params.spans.len(),
            pretty_number(analysis.params.total_span()),
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
        let is_simple = analysis.params.spans.len() == 1
            && analysis.left.restrains_vertical()
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
        let load_sample = analysis
            .solver
            .diagram
            .iter()
            .min_by(|left, right| {
                (left.x_m - analysis.params.load_position)
                    .abs()
                    .total_cmp(&(right.x_m - analysis.params.load_position).abs())
            })
            .copied();
        let node_v = load_sample.map_or(0.0, |sample| sample.displacement_m * 1_000.0);
        let node_theta = load_sample.map_or(0.0, |sample| sample.rotation_rad * 1_000.0);
        let total_dofs = analysis.solver.mesh_node_count * 2;

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
                        "分析网格",
                        format!("{} 节点", analysis.solver.mesh_node_count),
                        format!("总自由度 {total_dofs}"),
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
                        format!("{:.2e} N", analysis.solver.equilibrium_residual_n),
                        analysis.validation.summary(),
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
        let dirty_marker = if self.is_dirty() { "● " } else { "" };
        window.set_window_title(&format!(
            "{dirty_marker}{} · BridgeLab",
            self.params.project_name
        ));

        div()
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(Self::on_global_key))
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

fn build_model(params: &BeamParams, left: Support, right: Support) -> Result<BridgeModel, String> {
    let mut model = BridgeModel::continuous_beam(
        params.project_name.clone(),
        &params.spans,
        params.elastic_modulus * 1.0e9,
        params.area_m2,
        params.inertia_millionth * 1.0e-6,
    )
    .map_err(|error| error.to_string())?;
    let mut parts: ModelParts = model.parts().clone();
    parts.materials[0].name.clone_from(&params.material_name);
    parts.materials[0].density_kg_m3 = params.density_kg_m3;
    parts.sections[0].name.clone_from(&params.section_name);
    parts.load_cases[0].name.clone_from(&params.load_case_name);
    let first_node = parts.nodes[0].id;
    let last_node = parts.nodes[parts.nodes.len() - 1].id;
    if left == Support::Fixed && right == Support::Free {
        parts.supports.clear();
        parts.supports.push(bridge_core::Support {
            node: first_node,
            vertical: true,
            rotation: true,
        });
    } else {
        for support in &mut parts.supports {
            if support.node == first_node {
                support.vertical = left.restrains_vertical();
                support.rotation = left.restrains_rotation();
            } else if support.node == last_node {
                support.vertical = right.restrains_vertical();
                support.rotation = right.restrains_rotation();
            }
        }
        parts
            .supports
            .retain(|support| support.vertical || support.rotation);
    }
    model = BridgeModel::from_parts(parts).map_err(|error| error.to_string())?;
    model
        .set_primary_point_load(params.load_position, params.load * 1_000.0)
        .map_err(|error| error.to_string())?;
    Ok(model)
}

fn analyze(params: BeamParams, left: Support, right: Support) -> Result<Analysis, String> {
    let model = build_model(&params, left, right)?;
    let solver = solve(
        &model,
        SolveOptions {
            samples_per_segment: 28,
            ..SolveOptions::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let validation = validate_result(&model, &solver);
    if !validation.passed() {
        return Err(validation.summary());
    }

    let total_span = params.total_span();
    let load_position = params.load_position.clamp(0.0, total_span);
    let point_load_n = params.load * 1_000.0;
    let simple_left_reaction = point_load_n * (total_span - load_position) / total_span;
    let samples = solver
        .diagram
        .iter()
        .map(|point| {
            let applied_load = if point.x_m + total_span * 1.0e-12 >= load_position {
                point_load_n
            } else {
                0.0
            };
            let base_moment = simple_left_reaction * point.x_m
                - applied_load * (point.x_m - load_position).max(0.0);
            Sample {
                x: point.x_m,
                shear: point.shear_n,
                moment: point.moment_nm,
                base_moment,
                correction_moment: point.moment_nm - base_moment,
                displacement: point.displacement_m,
            }
        })
        .collect::<Vec<_>>();
    let reactions = solver
        .node_results
        .iter()
        .flat_map(|node| [node.reaction_vertical_n, node.reaction_moment_nm])
        .collect::<Vec<_>>();

    Ok(Analysis {
        params,
        left,
        right,
        model,
        validation,
        reactions,
        samples,
        max_shear: solver.max_abs_shear_n,
        max_moment: solver.max_abs_moment_nm,
        max_displacement: solver.max_abs_displacement_m,
        indeterminacy: solver.static_indeterminacy,
        solver,
    })
}

fn pretty_number(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn replace_field_range(
    value: &mut String,
    range: Range<usize>,
    text: &str,
    span_layout: bool,
) -> usize {
    let max_len: usize = if span_layout { 64 } else { 16 };
    let remaining = value.len() - range.len();
    let available = max_len.saturating_sub(remaining);
    let insert = text
        .chars()
        .filter_map(|character| match character {
            character if character.is_ascii_digit() || character == '.' || character == '-' => {
                Some(character)
            }
            ',' | ';' | '，' | '；' if span_layout => Some(','),
            character if character.is_ascii_whitespace() && span_layout => Some(' '),
            _ => None,
        })
        .take(available)
        .collect::<String>();
    value.replace_range(range.clone(), &insert);
    range.start + insert.len()
}

fn is_field_character(character: char, span_layout: bool) -> bool {
    character.is_ascii_digit()
        || character == '.'
        || character == '-'
        || (span_layout
            && (matches!(character, ',' | ';' | '，' | '；') || character.is_ascii_whitespace()))
}

fn parse_spans(source: &str) -> Result<Vec<f64>, String> {
    let spans = source
        .split(|character: char| {
            character == ','
                || character == ';'
                || character == '，'
                || character == '；'
                || character.is_whitespace()
        })
        .filter(|part| !part.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "跨径布置示例：8, 10, 8".to_string())?;
    if spans.is_empty() {
        Err("至少输入一个跨径，例如 8 或 8, 10, 8".to_string())
    } else {
        Ok(spans)
    }
}

fn editor_state_from_model(model: &BridgeModel) -> Result<(BeamParams, Support, Support), String> {
    model.validate().map_err(|error| error.to_string())?;
    if model.load_cases().len() != 1 || model.materials().len() != 1 || model.sections().len() != 1
    {
        return Err("快速编辑器要求单一荷载工况、材料和截面；原文件未被修改".to_string());
    }
    let load_case = model.active_load_case();
    if load_case.point_loads.len() != 1 || !load_case.distributed_loads.is_empty() {
        return Err("当前图形编辑器仅支持一个集中力；文件中的其他荷载不会被静默丢弃".to_string());
    }
    let load = &load_case.point_loads[0];
    if load.moment_nm.abs() > 1.0e-12 || load.force_down_n <= 0.0 {
        return Err("当前图形编辑器仅支持一个竖直向下的集中力".to_string());
    }
    let element = model
        .element(load.element)
        .ok_or_else(|| "集中力引用了不存在的单元".to_string())?;
    let start = model
        .node(element.start)
        .ok_or_else(|| "单元起点不存在".to_string())?;
    let end = model
        .node(element.end)
        .ok_or_else(|| "单元终点不存在".to_string())?;
    let load_global_x =
        start.position.x_m + load.relative_position * (end.position.x_m - start.position.x_m);
    let material = model
        .materials()
        .first()
        .ok_or_else(|| "工程缺少材料".to_string())?;
    let section = model
        .sections()
        .first()
        .ok_or_else(|| "工程缺少截面".to_string())?;
    let first_node = model
        .nodes()
        .first()
        .ok_or_else(|| "工程缺少节点".to_string())?;
    let last_node = model
        .nodes()
        .last()
        .ok_or_else(|| "工程缺少节点".to_string())?;
    if model
        .elements()
        .iter()
        .any(|element| element.material != material.id || element.section != section.id)
    {
        return Err("快速编辑器暂不修改分单元材料或截面；原文件未被修改".to_string());
    }
    let left = display_support(model, first_node.id, true)?;
    let right = display_support(model, last_node.id, false)?;

    let is_cantilever = left == Support::Fixed && right == Support::Free;
    if is_cantilever {
        if model
            .supports()
            .iter()
            .any(|support| support.node != first_node.id)
        {
            return Err("悬臂快速编辑模式不允许额外中间支座；原文件未被修改".to_string());
        }
    } else if model
        .nodes()
        .iter()
        .skip(1)
        .take(model.nodes().len().saturating_sub(2))
        .any(|node| {
            !model
                .supports()
                .iter()
                .any(|support| support.node == node.id && support.vertical && !support.rotation)
        })
    {
        return Err("当前图形编辑器要求中间桥墩为竖向滚动支座".to_string());
    }

    let params = BeamParams {
        project_name: model.name().to_string(),
        spans: model.spans_m(),
        load: load.force_down_n / 1_000.0,
        load_position: load_global_x - first_node.position.x_m,
        elastic_modulus: material.elastic_modulus_pa / 1.0e9,
        inertia_millionth: section.inertia_m4 / 1.0e-6,
        area_m2: section.area_m2,
        density_kg_m3: material.density_kg_m3,
        material_name: material.name.clone(),
        section_name: section.name.clone(),
        load_case_name: load_case.name.clone(),
    };
    let canonical = build_model(&params, left, right)?;
    if canonical != *model {
        return Err(
            "该文件包含快速编辑器无法无损表达的 ID、标签或模型属性；原文件未被修改".to_string(),
        );
    }
    Ok((params, left, right))
}

fn display_support(
    model: &BridgeModel,
    node: bridge_core::NodeId,
    is_left: bool,
) -> Result<Support, String> {
    let Some(support) = model.supports().iter().find(|support| support.node == node) else {
        return Ok(Support::Free);
    };
    match (support.vertical, support.rotation) {
        (true, true) => Ok(Support::Fixed),
        (true, false) if is_left => Ok(Support::Pinned),
        (true, false) => Ok(Support::Roller),
        (false, true) => Err("快速编辑器不支持仅约束转角的端部支座".to_string()),
        (false, false) => Err("工程包含无效的空支座定义".to_string()),
    }
}

fn safe_file_stem(name: &str) -> String {
    let stem = name
        .chars()
        .filter_map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                Some(character)
            } else if character.is_whitespace() {
                Some('_')
            } else {
                None
            }
        })
        .take(48)
        .collect::<String>();
    if stem.is_empty() {
        "BridgeLab工程".to_string()
    } else {
        stem
    }
}

fn display_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("工程文件")
        .to_string()
}

fn coordinated_save(
    coordinator: &SaveCoordinator,
    generation: u64,
    path: &Path,
    model: &BridgeModel,
) -> Result<SaveTaskOutcome, String> {
    let _guard = coordinator
        .lock
        .lock()
        .map_err(|_| "保存队列锁已损坏".to_string())?;
    if coordinator.generation.load(Ordering::Acquire) != generation {
        return Ok(SaveTaskOutcome::Superseded);
    }
    save_project(path, model)
        .map_err(|error| error.to_string())
        .map(|()| SaveTaskOutcome::Saved)
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
    let load_x = left
        + (right - left) * (analysis.params.load_position / analysis.params.total_span()) as f32;

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
    for node in analysis
        .model
        .nodes()
        .iter()
        .skip(1)
        .take(analysis.model.nodes().len().saturating_sub(2))
    {
        if analysis
            .model
            .supports()
            .iter()
            .any(|support| support.node == node.id && support.vertical)
        {
            let x =
                left + (right - left) * (node.position.x_m / analysis.params.total_span()) as f32;
            paint_support(window, x, beam_y, Support::Roller, false);
        }
    }
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
    let mut dimension_ticks = analysis
        .model
        .nodes()
        .iter()
        .map(|node| {
            left + (right - left) * (node.position.x_m / analysis.params.total_span()) as f32
        })
        .collect::<Vec<_>>();
    dimension_ticks.push(load_x);
    for x in dimension_ticks {
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
    let span = analysis.params.total_span();
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
    for node in analysis
        .model
        .nodes()
        .iter()
        .skip(1)
        .take(analysis.model.nodes().len().saturating_sub(2))
    {
        let x = left + (right - left) * (node.position.x_m / span) as f32;
        paint_vertical_line(window, x, top, bottom, 1.2, with_alpha(CYAN, 0.18));
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

    let load_x = left
        + (right - left) * (analysis.params.load_position / analysis.params.total_span()) as f32;
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
    let span = analysis.params.total_span();
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

    let load_x = left
        + (right - left) * (analysis.params.load_position / analysis.params.total_span()) as f32;
    paint_load_arrow(window, load_x, original_y);
    paint_support(window, left, original_y, analysis.left, true);
    paint_support(window, right, original_y, analysis.right, false);
    for node in analysis
        .model
        .nodes()
        .iter()
        .skip(1)
        .take(analysis.model.nodes().len().saturating_sub(2))
    {
        if analysis
            .model
            .supports()
            .iter()
            .any(|support| support.node == node.id && support.vertical)
        {
            let x =
                left + (right - left) * (node.position.x_m / analysis.params.total_span()) as f32;
            paint_support(window, x, original_y, Support::Roller, false);
        }
    }
}

pub fn run() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1360.0), px(860.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("BridgeLab · 多跨桥梁有限元".into()),
                    ..Default::default()
                }),
                window_decorations: Some(WindowDecorations::Server),
                window_min_size: Some(size(px(1060.0), px(680.0))),
                is_movable: true,
                is_resizable: true,
                app_id: Some("bridgelab".into()),
                ..Default::default()
            },
            |window, cx| {
                let app = cx.new(BeamLab::new);
                let weak_app = app.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    let dirty = weak_app
                        .read_with(cx, |app, _| app.is_dirty())
                        .unwrap_or(false);
                    if !dirty {
                        return true;
                    }
                    let prompt = window.prompt(
                        PromptLevel::Warning,
                        "工程仍有未保存更改",
                        Some("请取消后先保存，或确认放弃更改并关闭窗口。"),
                        &[PromptButton::ok("放弃并关闭"), PromptButton::cancel("取消")],
                        cx,
                    );
                    window
                        .spawn(cx, async move |cx| {
                            if prompt.await.unwrap_or(1) == 0 {
                                let _ = cx.update(|window, _| window.remove_window());
                            }
                        })
                        .detach();
                    false
                });
                app
            },
        )
        .expect("failed to open BridgeLab window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_input_replaces_only_the_selected_part() {
        let mut value = "3.2".to_string();

        let caret = replace_field_range(&mut value, 2..3, "75", false);

        assert_eq!(value, "3.75");
        assert_eq!(caret, 4);
    }

    #[test]
    fn numeric_paste_replaces_all_and_filters_units() {
        let mut value = "80".to_string();

        let caret = replace_field_range(&mut value, 0..2, " 120 kN.5\n", false);

        assert_eq!(value, "120.5");
        assert_eq!(caret, 5);
    }

    #[test]
    fn span_layout_accepts_common_separators() {
        let mut value = "8".to_string();
        replace_field_range(&mut value, 0..1, "8， 10; 12", true);

        assert_eq!(parse_spans(&value), Ok(vec![8.0, 10.0, 12.0]));
    }

    #[test]
    fn simply_supported_reactions_and_moment_are_correct() {
        let params = BeamParams {
            spans: vec![10.0],
            load: 100.0,
            load_position: 4.0,
            ..BeamParams::default()
        };
        let result = analyze(params, Support::Pinned, Support::Roller).expect("stable simple beam");
        assert!((result.left_reaction() - 60.0).abs() < 1.0e-8);
        assert!((result.right_reaction() - 40.0).abs() < 1.0e-8);
        assert!((result.max_moment / 1_000.0 - 240.0).abs() < 1.0e-5);
    }

    #[test]
    fn cantilever_tip_load_matches_closed_form_deflection() {
        let params = BeamParams {
            spans: vec![4.0],
            load: 20.0,
            load_position: 4.0,
            ..BeamParams::default()
        };
        let expected = params.load * 1_000.0 * params.total_span().powi(3) / (3.0 * params.ei());
        let result = analyze(params, Support::Fixed, Support::Free).expect("stable cantilever");
        assert!((result.max_displacement - expected).abs() < expected * 1.0e-8);
        assert!((result.left_reaction() - 20.0).abs() < 1.0e-8);
        assert!((result.left_reaction_moment() - 80.0).abs() < 1.0e-8);
    }

    #[test]
    fn a_single_pin_is_unstable() {
        let params = BeamParams {
            spans: vec![8.0],
            ..BeamParams::default()
        };
        let result = analyze(params, Support::Pinned, Support::Free);
        assert!(result.is_err());
    }

    #[test]
    fn canonical_project_can_be_reopened_without_data_loss() {
        let params = BeamParams::default();
        let model =
            build_model(&params, Support::Pinned, Support::Roller).expect("canonical model");

        let (restored, left, right) =
            editor_state_from_model(&model).expect("lossless editor projection");

        assert_eq!(restored, params);
        assert_eq!(left, Support::Pinned);
        assert_eq!(right, Support::Roller);
    }

    #[test]
    fn quick_editor_rejects_noncanonical_labels() {
        let params = BeamParams::default();
        let model =
            build_model(&params, Support::Pinned, Support::Roller).expect("canonical model");
        let mut parts = model.parts().clone();
        parts.nodes[0].label = "自定义节点".to_string();
        let model = BridgeModel::from_parts(parts).expect("valid domain model");

        assert!(editor_state_from_model(&model).is_err());
    }

    #[test]
    fn stale_save_generation_cannot_overwrite_a_newer_request() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("race.bridge.json");
        let params = BeamParams::default();
        let model =
            build_model(&params, Support::Pinned, Support::Roller).expect("canonical model");
        let coordinator = SaveCoordinator::default();
        let stale_generation = coordinator.begin();
        let current_generation = coordinator.begin();

        assert_eq!(
            coordinated_save(&coordinator, stale_generation, &path, &model),
            Ok(SaveTaskOutcome::Superseded)
        );
        assert!(!path.exists());
        assert_eq!(
            coordinated_save(&coordinator, current_generation, &path, &model),
            Ok(SaveTaskOutcome::Saved)
        );
        assert!(path.exists());
    }
}
