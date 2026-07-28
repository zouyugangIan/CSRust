//! Versioned, atomic project persistence for BridgeLab.

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use atomic_write_file::AtomicWriteFile;
use bridge_core::{
    BeamElement, BridgeModel, DistributedLoad, ElementId, LoadCase, LoadCaseId, LoadId, Material,
    MaterialId, ModelError, ModelParts, Node, NodeId, Point2, PointLoad, Section, SectionId,
    Support,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const PROJECT_EXTENSION: &str = "bridge.json";
pub const MAX_PROJECT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedProject {
    pub model: BridgeModel,
    pub migrated_from: Option<u32>,
}

#[derive(Debug, Error)]
pub enum ProjectIoError {
    #[error("无法读写工程文件：{0}")]
    Io(#[from] std::io::Error),
    #[error("工程文件不是有效 JSON：{0}")]
    Json(#[from] serde_json::Error),
    #[error("不支持工程文件版本 {found}，当前版本为 {current}")]
    UnsupportedSchema { found: u64, current: u32 },
    #[error("工程模型无效：{0}")]
    InvalidModel(#[from] ModelError),
    #[error("旧版工程数据无效：{0}")]
    InvalidLegacy(String),
    #[error("工程文件大小 {bytes} 字节，超过上限 {maximum} 字节")]
    ProjectTooLarge { bytes: u64, maximum: u64 },
    #[error("schema_version 必须是非负整数")]
    InvalidSchemaVersion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DocumentV1 {
    schema_version: u32,
    #[serde(default)]
    application: String,
    #[serde(default)]
    saved_at_unix_ms: u128,
    project: ProjectDto,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProjectDto {
    name: String,
    nodes: Vec<NodeDto>,
    elements: Vec<ElementDto>,
    materials: Vec<MaterialDto>,
    sections: Vec<SectionDto>,
    supports: Vec<SupportDto>,
    load_cases: Vec<LoadCaseDto>,
    active_load_case_id: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct NodeDto {
    id: u64,
    x_m: f64,
    y_m: f64,
    label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ElementDto {
    id: u64,
    start_node_id: u64,
    end_node_id: u64,
    material_id: u64,
    section_id: u64,
    label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MaterialDto {
    id: u64,
    name: String,
    elastic_modulus_pa: f64,
    density_kg_m3: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SectionDto {
    id: u64,
    name: String,
    area_m2: f64,
    inertia_m4: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SupportDto {
    node_id: u64,
    vertical: bool,
    rotation: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LoadCaseDto {
    id: u64,
    name: String,
    point_loads: Vec<PointLoadDto>,
    distributed_loads: Vec<DistributedLoadDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PointLoadDto {
    id: u64,
    element_id: u64,
    relative_position: f64,
    force_down_n: f64,
    moment_nm: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DistributedLoadDto {
    id: u64,
    element_id: u64,
    start_down_n_per_m: f64,
    end_down_n_per_m: f64,
}

#[derive(Clone, Debug, Deserialize)]
struct LegacyDocumentV0 {
    #[serde(default)]
    schema_version: u32,
    #[serde(default = "legacy_default_name")]
    name: String,
    span_m: f64,
    load_kn: f64,
    load_position_m: f64,
    elastic_modulus_gpa: f64,
    inertia_millionth_m4: f64,
}

pub fn load_project(path: impl AsRef<Path>) -> Result<LoadedProject, ProjectIoError> {
    let path = path.as_ref();
    let file = fs::File::open(path)?;
    let bytes = file.metadata()?.len();
    if bytes > MAX_PROJECT_BYTES {
        return Err(ProjectIoError::ProjectTooLarge {
            bytes,
            maximum: MAX_PROJECT_BYTES,
        });
    }
    let mut source = String::with_capacity(bytes as usize);
    file.take(MAX_PROJECT_BYTES + 1)
        .read_to_string(&mut source)?;
    let bytes = source.len() as u64;
    if bytes > MAX_PROJECT_BYTES {
        return Err(ProjectIoError::ProjectTooLarge {
            bytes,
            maximum: MAX_PROJECT_BYTES,
        });
    }
    from_json_str(&source)
}

pub fn save_project(path: impl AsRef<Path>, model: &BridgeModel) -> Result<(), ProjectIoError> {
    let requested_path = path.as_ref();
    let resolved_path = if fs::symlink_metadata(requested_path)
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        fs::canonicalize(requested_path)?
    } else {
        requested_path.to_path_buf()
    };
    let path = resolved_path.as_path();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let json = to_json_string(model)?;
    let bytes = json.len().saturating_add(1) as u64;
    if bytes > MAX_PROJECT_BYTES {
        return Err(ProjectIoError::ProjectTooLarge {
            bytes,
            maximum: MAX_PROJECT_BYTES,
        });
    }
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(json.as_bytes())?;
    file.write_all(b"\n")?;
    file.commit()?;
    Ok(())
}

pub fn from_json_str(source: &str) -> Result<LoadedProject, ProjectIoError> {
    let value: Value = serde_json::from_str(source)?;
    let schema_version = match value.get("schema_version") {
        Some(version) => version
            .as_u64()
            .ok_or(ProjectIoError::InvalidSchemaVersion)?,
        None => 0,
    };
    match schema_version {
        0 => migrate_v0(serde_json::from_value(value)?),
        1 => {
            let document: DocumentV1 = serde_json::from_value(value)?;
            Ok(LoadedProject {
                model: BridgeModel::from_parts(document.project.into_parts())?,
                migrated_from: None,
            })
        }
        found => Err(ProjectIoError::UnsupportedSchema {
            found,
            current: CURRENT_SCHEMA_VERSION,
        }),
    }
}

pub fn to_json_string(model: &BridgeModel) -> Result<String, ProjectIoError> {
    model.validate()?;
    let saved_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let document = DocumentV1 {
        schema_version: CURRENT_SCHEMA_VERSION,
        application: format!("BridgeLab {}", env!("CARGO_PKG_VERSION")),
        saved_at_unix_ms,
        project: ProjectDto::from_model(model),
    };
    Ok(serde_json::to_string_pretty(&document)?)
}

#[must_use]
pub fn ensure_project_extension(path: impl Into<PathBuf>) -> PathBuf {
    let mut path = path.into();
    let has_bridge_json = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".bridge.json"));
    if !has_bridge_json {
        path.set_extension(PROJECT_EXTENSION);
    }
    path
}

impl ProjectDto {
    fn from_model(model: &BridgeModel) -> Self {
        let parts = model.parts();
        Self {
            name: parts.name.clone(),
            nodes: parts
                .nodes
                .iter()
                .map(|node| NodeDto {
                    id: node.id.get(),
                    x_m: node.position.x_m,
                    y_m: node.position.y_m,
                    label: node.label.clone(),
                })
                .collect(),
            elements: parts
                .elements
                .iter()
                .map(|element| ElementDto {
                    id: element.id.get(),
                    start_node_id: element.start.get(),
                    end_node_id: element.end.get(),
                    material_id: element.material.get(),
                    section_id: element.section.get(),
                    label: element.label.clone(),
                })
                .collect(),
            materials: parts
                .materials
                .iter()
                .map(|material| MaterialDto {
                    id: material.id.get(),
                    name: material.name.clone(),
                    elastic_modulus_pa: material.elastic_modulus_pa,
                    density_kg_m3: material.density_kg_m3,
                })
                .collect(),
            sections: parts
                .sections
                .iter()
                .map(|section| SectionDto {
                    id: section.id.get(),
                    name: section.name.clone(),
                    area_m2: section.area_m2,
                    inertia_m4: section.inertia_m4,
                })
                .collect(),
            supports: parts
                .supports
                .iter()
                .map(|support| SupportDto {
                    node_id: support.node.get(),
                    vertical: support.vertical,
                    rotation: support.rotation,
                })
                .collect(),
            load_cases: parts
                .load_cases
                .iter()
                .map(|load_case| LoadCaseDto {
                    id: load_case.id.get(),
                    name: load_case.name.clone(),
                    point_loads: load_case
                        .point_loads
                        .iter()
                        .map(|load| PointLoadDto {
                            id: load.id.get(),
                            element_id: load.element.get(),
                            relative_position: load.relative_position,
                            force_down_n: load.force_down_n,
                            moment_nm: load.moment_nm,
                        })
                        .collect(),
                    distributed_loads: load_case
                        .distributed_loads
                        .iter()
                        .map(|load| DistributedLoadDto {
                            id: load.id.get(),
                            element_id: load.element.get(),
                            start_down_n_per_m: load.start_down_n_per_m,
                            end_down_n_per_m: load.end_down_n_per_m,
                        })
                        .collect(),
                })
                .collect(),
            active_load_case_id: parts.active_load_case.get(),
        }
    }

    fn into_parts(self) -> ModelParts {
        ModelParts {
            name: self.name,
            nodes: self
                .nodes
                .into_iter()
                .map(|node| Node {
                    id: NodeId::new(node.id),
                    position: Point2 {
                        x_m: node.x_m,
                        y_m: node.y_m,
                    },
                    label: node.label,
                })
                .collect(),
            elements: self
                .elements
                .into_iter()
                .map(|element| BeamElement {
                    id: ElementId::new(element.id),
                    start: NodeId::new(element.start_node_id),
                    end: NodeId::new(element.end_node_id),
                    material: MaterialId::new(element.material_id),
                    section: SectionId::new(element.section_id),
                    label: element.label,
                })
                .collect(),
            materials: self
                .materials
                .into_iter()
                .map(|material| Material {
                    id: MaterialId::new(material.id),
                    name: material.name,
                    elastic_modulus_pa: material.elastic_modulus_pa,
                    density_kg_m3: material.density_kg_m3,
                })
                .collect(),
            sections: self
                .sections
                .into_iter()
                .map(|section| Section {
                    id: SectionId::new(section.id),
                    name: section.name,
                    area_m2: section.area_m2,
                    inertia_m4: section.inertia_m4,
                })
                .collect(),
            supports: self
                .supports
                .into_iter()
                .map(|support| Support {
                    node: NodeId::new(support.node_id),
                    vertical: support.vertical,
                    rotation: support.rotation,
                })
                .collect(),
            load_cases: self
                .load_cases
                .into_iter()
                .map(|load_case| LoadCase {
                    id: LoadCaseId::new(load_case.id),
                    name: load_case.name,
                    point_loads: load_case
                        .point_loads
                        .into_iter()
                        .map(|load| PointLoad {
                            id: LoadId::new(load.id),
                            element: ElementId::new(load.element_id),
                            relative_position: load.relative_position,
                            force_down_n: load.force_down_n,
                            moment_nm: load.moment_nm,
                        })
                        .collect(),
                    distributed_loads: load_case
                        .distributed_loads
                        .into_iter()
                        .map(|load| DistributedLoad {
                            id: LoadId::new(load.id),
                            element: ElementId::new(load.element_id),
                            start_down_n_per_m: load.start_down_n_per_m,
                            end_down_n_per_m: load.end_down_n_per_m,
                        })
                        .collect(),
                })
                .collect(),
            active_load_case: LoadCaseId::new(self.active_load_case_id),
        }
    }
}

fn migrate_v0(legacy: LegacyDocumentV0) -> Result<LoadedProject, ProjectIoError> {
    if legacy.schema_version != 0 {
        return Err(ProjectIoError::InvalidLegacy(
            "旧版 schema_version 必须为 0".to_string(),
        ));
    }
    let mut model = BridgeModel::continuous_beam(
        legacy.name,
        &[legacy.span_m],
        legacy.elastic_modulus_gpa * 1.0e9,
        0.12,
        legacy.inertia_millionth_m4 * 1.0e-6,
    )?;
    model.set_primary_point_load(legacy.load_position_m, legacy.load_kn * 1_000.0)?;
    Ok(LoadedProject {
        model,
        migrated_from: Some(0),
    })
}

fn legacy_default_name() -> String {
    "迁移的单跨梁工程".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_model() -> BridgeModel {
        let mut model =
            BridgeModel::continuous_beam("三跨连续梁", &[8.0, 10.0, 8.0], 200.0e9, 0.12, 8.0e-6)
                .expect("valid model");
        model
            .set_primary_point_load(12.0, 80_000.0)
            .expect("valid load");
        model
    }

    #[test]
    fn current_schema_round_trips_without_domain_loss() {
        let model = demo_model();
        let source = to_json_string(&model).expect("serialize project");
        let loaded = from_json_str(&source).expect("load project");

        assert_eq!(loaded.model, model);
        assert_eq!(loaded.migrated_from, None);
    }

    #[test]
    fn schema_metadata_is_optional_within_version_one() {
        let model = demo_model();
        let source = to_json_string(&model).expect("serialize project");
        let mut value: Value = serde_json::from_str(&source).expect("valid JSON");
        let object = value.as_object_mut().expect("document object");
        object.remove("application");
        object.remove("saved_at_unix_ms");

        let loaded = from_json_str(&value.to_string()).expect("load metadata-light project");

        assert_eq!(loaded.model, model);
    }

    #[test]
    fn save_is_atomic_and_loadable() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("demo.bridge.json");
        let model = demo_model();

        save_project(&path, &model).expect("atomic save");
        let loaded = load_project(&path).expect("read saved project");

        assert_eq!(loaded.model, model);
    }

    #[test]
    fn v0_single_span_project_is_migrated() {
        let source = r#"{
            "schema_version": 0,
            "name": "旧工程",
            "span_m": 10.0,
            "load_kn": 100.0,
            "load_position_m": 4.0,
            "elastic_modulus_gpa": 200.0,
            "inertia_millionth_m4": 8.0
        }"#;

        let loaded = from_json_str(source).expect("migrate legacy project");

        assert_eq!(loaded.migrated_from, Some(0));
        assert_eq!(loaded.model.spans_m(), vec![10.0]);
        assert_eq!(
            loaded.model.active_load_case().point_loads[0].force_down_n,
            100_000.0
        );
    }

    #[test]
    fn future_schema_is_rejected_explicitly() {
        let source = r#"{"schema_version": 99}"#;

        let error = from_json_str(source).expect_err("future schema must fail");

        assert!(matches!(
            error,
            ProjectIoError::UnsupportedSchema {
                found: 99,
                current: CURRENT_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn malformed_schema_version_is_rejected_explicitly() {
        let error = from_json_str(r#"{"schema_version":"1"}"#)
            .expect_err("string schema version must fail");

        assert!(matches!(error, ProjectIoError::InvalidSchemaVersion));
    }

    #[test]
    fn oversized_project_is_rejected_before_reading() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("oversized.bridge.json");
        let file = fs::File::create(&path).expect("create sparse project");
        file.set_len(MAX_PROJECT_BYTES + 1)
            .expect("extend sparse project");

        let error = load_project(path).expect_err("oversized project must fail");

        assert!(matches!(
            error,
            ProjectIoError::ProjectTooLarge {
                bytes,
                maximum: MAX_PROJECT_BYTES
            } if bytes == MAX_PROJECT_BYTES + 1
        ));
    }

    #[test]
    fn extension_is_normalized_only_once() {
        assert_eq!(
            ensure_project_extension("demo"),
            PathBuf::from("demo.bridge.json")
        );
        assert_eq!(
            ensure_project_extension("demo.bridge.json"),
            PathBuf::from("demo.bridge.json")
        );
    }
}
