//! Domain model for small two-dimensional bridge beam projects.
//!
//! The crate intentionally has no UI, persistence, or matrix-solver dependency.
//! All public dimensional values include their SI unit in the field name.

use std::collections::HashSet;

use thiserror::Error;

const POSITION_TOLERANCE_M: f64 = 1.0e-9;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(NodeId);
id_type!(ElementId);
id_type!(MaterialId);
id_type!(SectionId);
id_type!(LoadCaseId);
id_type!(LoadId);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2 {
    pub x_m: f64,
    pub y_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub position: Point2,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub id: MaterialId,
    pub name: String,
    pub elastic_modulus_pa: f64,
    pub density_kg_m3: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub id: SectionId,
    pub name: String,
    pub area_m2: f64,
    pub inertia_m4: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamElement {
    pub id: ElementId,
    pub start: NodeId,
    pub end: NodeId,
    pub material: MaterialId,
    pub section: SectionId,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Support {
    pub node: NodeId,
    pub vertical: bool,
    pub rotation: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PointLoad {
    pub id: LoadId,
    pub element: ElementId,
    /// Position along the element, in the closed range `0.0..=1.0`.
    pub relative_position: f64,
    /// Downward force is positive.
    pub force_down_n: f64,
    /// Counter-clockwise nodal moment is positive.
    pub moment_nm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DistributedLoad {
    pub id: LoadId,
    pub element: ElementId,
    /// Downward load intensity at the element start.
    pub start_down_n_per_m: f64,
    /// Downward load intensity at the element end.
    pub end_down_n_per_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadCase {
    pub id: LoadCaseId,
    pub name: String,
    pub point_loads: Vec<PointLoad>,
    pub distributed_loads: Vec<DistributedLoad>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelParts {
    pub name: String,
    pub nodes: Vec<Node>,
    pub elements: Vec<BeamElement>,
    pub materials: Vec<Material>,
    pub sections: Vec<Section>,
    pub supports: Vec<Support>,
    pub load_cases: Vec<LoadCase>,
    pub active_load_case: LoadCaseId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BridgeModel {
    parts: ModelParts,
    next_id: u64,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ModelError {
    #[error("桥梁名称不能为空")]
    EmptyName,
    #[error("至少需要一个有效跨径")]
    EmptySpans,
    #[error("跨径必须是有限且大于 0.05 m 的数值")]
    InvalidSpan,
    #[error("弹性模量必须是有限正数")]
    InvalidElasticModulus,
    #[error("截面面积必须是有限正数")]
    InvalidArea,
    #[error("截面惯性矩必须是有限正数")]
    InvalidInertia,
    #[error("材料密度必须是有限且非负的数值")]
    InvalidDensity,
    #[error("模型中的 ID 必须唯一")]
    DuplicateId,
    #[error("单元引用了不存在的节点、材料或截面")]
    DanglingElementReference,
    #[error("支座引用了不存在的节点")]
    DanglingSupportReference,
    #[error("同一节点不能定义多个支座")]
    DuplicateSupport,
    #[error("荷载工况或荷载引用无效")]
    DanglingLoadReference,
    #[error("当前荷载工况不存在")]
    MissingActiveLoadCase,
    #[error("当前简化求解器仅支持从左到右的水平连续梁")]
    UnsupportedGeometry,
    #[error("荷载位置必须位于桥梁范围内")]
    LoadOutsideBridge,
    #[error("荷载数值必须有限")]
    InvalidLoad,
}

impl BridgeModel {
    /// Creates a horizontal continuous beam with a vertical support at every
    /// span boundary and a single active load case.
    pub fn continuous_beam(
        name: impl Into<String>,
        spans_m: &[f64],
        elastic_modulus_pa: f64,
        area_m2: f64,
        inertia_m4: f64,
    ) -> Result<Self, ModelError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(ModelError::EmptyName);
        }
        if spans_m.is_empty() {
            return Err(ModelError::EmptySpans);
        }
        if spans_m
            .iter()
            .any(|span| !span.is_finite() || *span <= 0.05)
        {
            return Err(ModelError::InvalidSpan);
        }
        if !elastic_modulus_pa.is_finite() || elastic_modulus_pa <= 0.0 {
            return Err(ModelError::InvalidElasticModulus);
        }
        if !area_m2.is_finite() || area_m2 <= 0.0 {
            return Err(ModelError::InvalidArea);
        }
        if !inertia_m4.is_finite() || inertia_m4 <= 0.0 {
            return Err(ModelError::InvalidInertia);
        }

        let material_id = MaterialId::new(1);
        let section_id = SectionId::new(2);
        let load_case_id = LoadCaseId::new(3);
        let mut next_id = 4;
        let mut x_m = 0.0;
        let mut nodes = Vec::with_capacity(spans_m.len() + 1);
        for index in 0..=spans_m.len() {
            let id = NodeId::new(next_id);
            next_id += 1;
            nodes.push(Node {
                id,
                position: Point2 { x_m, y_m: 0.0 },
                label: format!("N{}", index + 1),
            });
            if let Some(span) = spans_m.get(index) {
                x_m += span;
            }
        }

        let mut elements = Vec::with_capacity(spans_m.len());
        for index in 0..spans_m.len() {
            elements.push(BeamElement {
                id: ElementId::new(next_id),
                start: nodes[index].id,
                end: nodes[index + 1].id,
                material: material_id,
                section: section_id,
                label: format!("E{}", index + 1),
            });
            next_id += 1;
        }

        let supports = nodes
            .iter()
            .map(|node| Support {
                node: node.id,
                vertical: true,
                rotation: false,
            })
            .collect();

        let parts = ModelParts {
            name,
            nodes,
            elements,
            materials: vec![Material {
                id: material_id,
                name: "主梁材料".to_string(),
                elastic_modulus_pa,
                density_kg_m3: 7_850.0,
            }],
            sections: vec![Section {
                id: section_id,
                name: "主梁截面".to_string(),
                area_m2,
                inertia_m4,
            }],
            supports,
            load_cases: vec![LoadCase {
                id: load_case_id,
                name: "LC1 恒载".to_string(),
                point_loads: Vec::new(),
                distributed_loads: Vec::new(),
            }],
            active_load_case: load_case_id,
        };
        Self::from_parts(parts)
    }

    pub fn from_parts(parts: ModelParts) -> Result<Self, ModelError> {
        validate_parts(&parts)?;
        let next_id = maximum_id(&parts).saturating_add(1).max(1);
        Ok(Self { parts, next_id })
    }

    #[must_use]
    pub fn parts(&self) -> &ModelParts {
        &self.parts
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.parts.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) -> Result<(), ModelError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(ModelError::EmptyName);
        }
        self.parts.name = name;
        Ok(())
    }

    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.parts.nodes
    }

    #[must_use]
    pub fn elements(&self) -> &[BeamElement] {
        &self.parts.elements
    }

    #[must_use]
    pub fn materials(&self) -> &[Material] {
        &self.parts.materials
    }

    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.parts.sections
    }

    #[must_use]
    pub fn supports(&self) -> &[Support] {
        &self.parts.supports
    }

    #[must_use]
    pub fn load_cases(&self) -> &[LoadCase] {
        &self.parts.load_cases
    }

    #[must_use]
    pub const fn active_load_case_id(&self) -> LoadCaseId {
        self.parts.active_load_case
    }

    #[must_use]
    pub fn active_load_case(&self) -> &LoadCase {
        self.parts
            .load_cases
            .iter()
            .find(|load_case| load_case.id == self.parts.active_load_case)
            .expect("validated model always contains its active load case")
    }

    #[must_use]
    pub fn total_length_m(&self) -> f64 {
        let first = self
            .parts
            .nodes
            .first()
            .map_or(0.0, |node| node.position.x_m);
        let last = self
            .parts
            .nodes
            .last()
            .map_or(0.0, |node| node.position.x_m);
        last - first
    }

    #[must_use]
    pub fn spans_m(&self) -> Vec<f64> {
        self.parts
            .nodes
            .windows(2)
            .map(|nodes| nodes[1].position.x_m - nodes[0].position.x_m)
            .collect()
    }

    pub fn set_primary_point_load(
        &mut self,
        global_x_m: f64,
        force_down_n: f64,
    ) -> Result<(), ModelError> {
        if !global_x_m.is_finite() || !force_down_n.is_finite() {
            return Err(ModelError::InvalidLoad);
        }
        let first_x = self
            .parts
            .nodes
            .first()
            .map_or(0.0, |node| node.position.x_m);
        let local_x = global_x_m - first_x;
        let total_length = self.total_length_m();
        if local_x < -POSITION_TOLERANCE_M || local_x > total_length + POSITION_TOLERANCE_M {
            return Err(ModelError::LoadOutsideBridge);
        }

        let x = global_x_m.clamp(first_x, first_x + total_length);
        let (element_id, relative_position) = self
            .parts
            .elements
            .iter()
            .find_map(|element| {
                let start = self.node(element.start)?;
                let end = self.node(element.end)?;
                let is_last = element.id == self.parts.elements.last()?.id;
                let contains = x >= start.position.x_m - POSITION_TOLERANCE_M
                    && (x < end.position.x_m - POSITION_TOLERANCE_M
                        || is_last
                        || (x - end.position.x_m).abs() <= POSITION_TOLERANCE_M);
                contains.then(|| {
                    let length = end.position.x_m - start.position.x_m;
                    (
                        element.id,
                        ((x - start.position.x_m) / length).clamp(0.0, 1.0),
                    )
                })
            })
            .ok_or(ModelError::LoadOutsideBridge)?;

        let id = LoadId::new(self.take_id());
        let active = self
            .parts
            .load_cases
            .iter_mut()
            .find(|load_case| load_case.id == self.parts.active_load_case)
            .ok_or(ModelError::MissingActiveLoadCase)?;
        active.point_loads.clear();
        active.point_loads.push(PointLoad {
            id,
            element: element_id,
            relative_position,
            force_down_n,
            moment_nm: 0.0,
        });
        Ok(())
    }

    pub fn set_uniform_properties(
        &mut self,
        elastic_modulus_pa: f64,
        area_m2: f64,
        inertia_m4: f64,
    ) -> Result<(), ModelError> {
        if !elastic_modulus_pa.is_finite() || elastic_modulus_pa <= 0.0 {
            return Err(ModelError::InvalidElasticModulus);
        }
        if !area_m2.is_finite() || area_m2 <= 0.0 {
            return Err(ModelError::InvalidArea);
        }
        if !inertia_m4.is_finite() || inertia_m4 <= 0.0 {
            return Err(ModelError::InvalidInertia);
        }
        for material in &mut self.parts.materials {
            material.elastic_modulus_pa = elastic_modulus_pa;
        }
        for section in &mut self.parts.sections {
            section.area_m2 = area_m2;
            section.inertia_m4 = inertia_m4;
        }
        Ok(())
    }

    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.parts.nodes.iter().find(|node| node.id == id)
    }

    #[must_use]
    pub fn element(&self, id: ElementId) -> Option<&BeamElement> {
        self.parts.elements.iter().find(|element| element.id == id)
    }

    #[must_use]
    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.parts
            .materials
            .iter()
            .find(|material| material.id == id)
    }

    #[must_use]
    pub fn section(&self, id: SectionId) -> Option<&Section> {
        self.parts.sections.iter().find(|section| section.id == id)
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        validate_parts(&self.parts)
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }
}

fn validate_parts(parts: &ModelParts) -> Result<(), ModelError> {
    if parts.name.trim().is_empty() {
        return Err(ModelError::EmptyName);
    }
    if parts.nodes.len() < 2 || parts.elements.is_empty() {
        return Err(ModelError::EmptySpans);
    }

    let mut ids = HashSet::new();
    let all_ids = parts
        .nodes
        .iter()
        .map(|item| item.id.get())
        .chain(parts.elements.iter().map(|item| item.id.get()))
        .chain(parts.materials.iter().map(|item| item.id.get()))
        .chain(parts.sections.iter().map(|item| item.id.get()))
        .chain(parts.load_cases.iter().map(|item| item.id.get()))
        .chain(
            parts
                .load_cases
                .iter()
                .flat_map(|item| item.point_loads.iter().map(|load| load.id.get())),
        )
        .chain(
            parts
                .load_cases
                .iter()
                .flat_map(|item| item.distributed_loads.iter().map(|load| load.id.get())),
        );
    for id in all_ids {
        if id == 0 || !ids.insert(id) {
            return Err(ModelError::DuplicateId);
        }
    }

    if parts.materials.iter().any(|material| {
        !material.elastic_modulus_pa.is_finite() || material.elastic_modulus_pa <= 0.0
    }) {
        return Err(ModelError::InvalidElasticModulus);
    }
    if parts
        .materials
        .iter()
        .any(|material| !material.density_kg_m3.is_finite() || material.density_kg_m3 < 0.0)
    {
        return Err(ModelError::InvalidDensity);
    }
    if parts
        .sections
        .iter()
        .any(|section| !section.area_m2.is_finite() || section.area_m2 <= 0.0)
    {
        return Err(ModelError::InvalidArea);
    }
    if parts
        .sections
        .iter()
        .any(|section| !section.inertia_m4.is_finite() || section.inertia_m4 <= 0.0)
    {
        return Err(ModelError::InvalidInertia);
    }

    let node_ids = parts
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<HashSet<_>>();
    let material_ids = parts
        .materials
        .iter()
        .map(|material| material.id)
        .collect::<HashSet<_>>();
    let section_ids = parts
        .sections
        .iter()
        .map(|section| section.id)
        .collect::<HashSet<_>>();
    let element_ids = parts
        .elements
        .iter()
        .map(|element| element.id)
        .collect::<HashSet<_>>();

    if parts.elements.iter().any(|element| {
        !node_ids.contains(&element.start)
            || !node_ids.contains(&element.end)
            || !material_ids.contains(&element.material)
            || !section_ids.contains(&element.section)
    }) {
        return Err(ModelError::DanglingElementReference);
    }
    if parts
        .supports
        .iter()
        .any(|support| !node_ids.contains(&support.node))
    {
        return Err(ModelError::DanglingSupportReference);
    }
    let mut support_nodes = HashSet::new();
    if parts
        .supports
        .iter()
        .any(|support| !support_nodes.insert(support.node))
    {
        return Err(ModelError::DuplicateSupport);
    }
    if !parts
        .load_cases
        .iter()
        .any(|load_case| load_case.id == parts.active_load_case)
    {
        return Err(ModelError::MissingActiveLoadCase);
    }
    if parts.load_cases.iter().any(|load_case| {
        load_case.point_loads.iter().any(|load| {
            !element_ids.contains(&load.element)
                || !load.relative_position.is_finite()
                || !(0.0..=1.0).contains(&load.relative_position)
                || !load.force_down_n.is_finite()
                || !load.moment_nm.is_finite()
        }) || load_case.distributed_loads.iter().any(|load| {
            !element_ids.contains(&load.element)
                || !load.start_down_n_per_m.is_finite()
                || !load.end_down_n_per_m.is_finite()
        })
    }) {
        return Err(ModelError::DanglingLoadReference);
    }

    let first_y = parts.nodes[0].position.y_m;
    if parts.nodes.iter().any(|node| {
        !node.position.x_m.is_finite()
            || !node.position.y_m.is_finite()
            || (node.position.y_m - first_y).abs() > POSITION_TOLERANCE_M
    }) || parts
        .nodes
        .windows(2)
        .any(|nodes| nodes[1].position.x_m - nodes[0].position.x_m <= 0.05)
    {
        return Err(ModelError::UnsupportedGeometry);
    }

    let expected_pairs = parts
        .nodes
        .windows(2)
        .map(|nodes| (nodes[0].id, nodes[1].id))
        .collect::<Vec<_>>();
    if parts.elements.len() != expected_pairs.len()
        || parts
            .elements
            .iter()
            .zip(expected_pairs)
            .any(|(element, pair)| (element.start, element.end) != pair)
    {
        return Err(ModelError::UnsupportedGeometry);
    }
    Ok(())
}

fn maximum_id(parts: &ModelParts) -> u64 {
    parts
        .nodes
        .iter()
        .map(|item| item.id.get())
        .chain(parts.elements.iter().map(|item| item.id.get()))
        .chain(parts.materials.iter().map(|item| item.id.get()))
        .chain(parts.sections.iter().map(|item| item.id.get()))
        .chain(parts.load_cases.iter().map(|item| item.id.get()))
        .chain(
            parts
                .load_cases
                .iter()
                .flat_map(|item| item.point_loads.iter().map(|load| load.id.get())),
        )
        .chain(
            parts
                .load_cases
                .iter()
                .flat_map(|item| item.distributed_loads.iter().map(|load| load.id.get())),
        )
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_beam_has_stable_typed_ids_and_spans() {
        let model =
            BridgeModel::continuous_beam("三跨桥", &[8.0, 10.0, 8.0], 200.0e9, 0.12, 8.0e-6)
                .expect("valid model");

        assert_eq!(model.nodes().len(), 4);
        assert_eq!(model.elements().len(), 3);
        assert_eq!(model.spans_m(), vec![8.0, 10.0, 8.0]);
        assert_eq!(model.total_length_m(), 26.0);
        assert!(model.validate().is_ok());
    }

    #[test]
    fn primary_load_is_located_on_the_correct_span() {
        let mut model =
            BridgeModel::continuous_beam("三跨桥", &[8.0, 10.0, 8.0], 200.0e9, 0.12, 8.0e-6)
                .expect("valid model");

        model
            .set_primary_point_load(12.0, 80_000.0)
            .expect("load within bridge");

        let load = &model.active_load_case().point_loads[0];
        assert_eq!(load.element, model.elements()[1].id);
        assert!((load.relative_position - 0.4).abs() < 1.0e-12);
    }

    #[test]
    fn future_model_cannot_hide_a_dangling_reference() {
        let model = BridgeModel::continuous_beam("一跨桥", &[8.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid model");
        let mut parts = model.parts().clone();
        parts.elements[0].material = MaterialId::new(99_999);

        assert_eq!(
            BridgeModel::from_parts(parts),
            Err(ModelError::DanglingElementReference)
        );
    }

    #[test]
    fn duplicate_support_definition_is_rejected() {
        let model = BridgeModel::continuous_beam("一跨桥", &[8.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid model");
        let mut parts = model.parts().clone();
        parts.supports.push(parts.supports[0].clone());

        assert_eq!(
            BridgeModel::from_parts(parts),
            Err(ModelError::DuplicateSupport)
        );
    }
}
