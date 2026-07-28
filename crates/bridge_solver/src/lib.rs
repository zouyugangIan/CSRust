//! Linear-static Euler–Bernoulli beam solver for [`bridge_core`] models.
//!
//! The matrix backend is abstracted so larger models can move to a sparse
//! implementation without leaking matrix details into the domain or UI crates.

use std::collections::HashMap;

use bridge_core::{BridgeModel, ElementId, LoadCaseId, NodeId};
use thiserror::Error;

const COORDINATE_TOLERANCE_M: f64 = 1.0e-9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolveOptions {
    pub samples_per_segment: usize,
    /// Guardrail for the built-in dense backend.
    pub max_degrees_of_freedom: usize,
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            samples_per_segment: 24,
            max_degrees_of_freedom: 4_096,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagramPoint {
    pub x_m: f64,
    pub shear_n: f64,
    pub moment_nm: f64,
    pub displacement_m: f64,
    pub rotation_rad: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeResult {
    pub node: NodeId,
    pub x_m: f64,
    pub displacement_m: f64,
    pub rotation_rad: f64,
    pub reaction_vertical_n: f64,
    pub reaction_moment_nm: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementResult {
    pub parent: ElementId,
    pub start_x_m: f64,
    pub end_x_m: f64,
    pub start_shear_n: f64,
    pub start_moment_nm: f64,
    pub end_shear_n: f64,
    pub end_moment_nm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisResult {
    pub load_case: LoadCaseId,
    pub total_length_m: f64,
    pub node_results: Vec<NodeResult>,
    pub element_results: Vec<ElementResult>,
    pub diagram: Vec<DiagramPoint>,
    pub max_abs_shear_n: f64,
    pub max_abs_moment_nm: f64,
    pub max_abs_displacement_m: f64,
    pub equilibrium_residual_n: f64,
    pub equilibrium_moment_residual_nm: f64,
    pub mesh_node_count: usize,
    pub static_indeterminacy: usize,
}

impl AnalysisResult {
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&NodeResult> {
        self.node_results.iter().find(|result| result.node == id)
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SolveError {
    #[error("模型无效：{0}")]
    InvalidModel(String),
    #[error("当前荷载工况不存在")]
    MissingLoadCase,
    #[error("单元 {0:?} 的长度或刚度无效")]
    InvalidElement(ElementId),
    #[error("约束不足或模型形成机构，刚度矩阵奇异")]
    SingularSystem,
    #[error("线性方程组包含非有限数值")]
    NonFiniteSystem,
    #[error("模型包含 {degrees_of_freedom} 个自由度，超过当前稠密后端上限 {maximum}")]
    ModelTooLarge {
        degrees_of_freedom: usize,
        maximum: usize,
    },
}

pub trait LinearSystemSolver: Send + Sync {
    fn solve(&self, matrix: Vec<Vec<f64>>, rhs: Vec<f64>) -> Result<Vec<f64>, SolveError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DensePivotSolver;

impl LinearSystemSolver for DensePivotSolver {
    fn solve(&self, mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Result<Vec<f64>, SolveError> {
        let size = rhs.len();
        if size == 0 {
            return Ok(Vec::new());
        }
        if matrix.len() != size
            || matrix.iter().any(|row| row.len() != size)
            || matrix.iter().flatten().any(|value| !value.is_finite())
            || rhs.iter().any(|value| !value.is_finite())
        {
            return Err(SolveError::NonFiniteSystem);
        }

        let mut row_scales = matrix
            .iter()
            .map(|row| row.iter().map(|value| value.abs()).fold(0.0, f64::max))
            .collect::<Vec<_>>();

        for pivot_column in 0..size {
            let pivot_row = (pivot_column..size)
                .max_by(|left, right| {
                    let left_ratio = if row_scales[*left] > 0.0 {
                        matrix[*left][pivot_column].abs() / row_scales[*left]
                    } else {
                        0.0
                    };
                    let right_ratio = if row_scales[*right] > 0.0 {
                        matrix[*right][pivot_column].abs() / row_scales[*right]
                    } else {
                        0.0
                    };
                    left_ratio.total_cmp(&right_ratio)
                })
                .ok_or(SolveError::SingularSystem)?;
            let tolerance = row_scales[pivot_row] * f64::EPSILON * size.max(1) as f64 * 64.0;
            if matrix[pivot_row][pivot_column].abs() <= tolerance {
                return Err(SolveError::SingularSystem);
            }
            if pivot_row != pivot_column {
                matrix.swap(pivot_row, pivot_column);
                rhs.swap(pivot_row, pivot_column);
                row_scales.swap(pivot_row, pivot_column);
            }

            let pivot_values = matrix[pivot_column].clone();
            let pivot_rhs = rhs[pivot_column];
            for (row_values, row_rhs) in
                matrix.iter_mut().zip(rhs.iter_mut()).skip(pivot_column + 1)
            {
                let factor = row_values[pivot_column] / pivot_values[pivot_column];
                row_values[pivot_column] = 0.0;
                for (value, pivot_value) in row_values
                    .iter_mut()
                    .zip(&pivot_values)
                    .skip(pivot_column + 1)
                {
                    *value -= factor * pivot_value;
                }
                *row_rhs -= factor * pivot_rhs;
            }
        }

        let mut solution = vec![0.0; size];
        for row in (0..size).rev() {
            let known = (row + 1..size)
                .map(|column| matrix[row][column] * solution[column])
                .sum::<f64>();
            let pivot = matrix[row][row];
            let tolerance = row_scales[row] * f64::EPSILON * size.max(1) as f64 * 64.0;
            if pivot.abs() <= tolerance {
                return Err(SolveError::SingularSystem);
            }
            solution[row] = (rhs[row] - known) / pivot;
        }
        if solution.iter().any(|value| !value.is_finite()) {
            return Err(SolveError::NonFiniteSystem);
        }
        Ok(solution)
    }
}

#[derive(Clone, Copy, Debug)]
struct MeshElement {
    parent: ElementId,
    start_index: usize,
    end_index: usize,
    start_x_m: f64,
    end_x_m: f64,
    flexural_rigidity_nm2: f64,
    start_load_down_n_per_m: f64,
    end_load_down_n_per_m: f64,
}

impl MeshElement {
    fn length_m(self) -> f64 {
        self.end_x_m - self.start_x_m
    }
}

#[derive(Clone, Debug)]
struct AnalysisMesh {
    coordinates_m: Vec<f64>,
    elements: Vec<MeshElement>,
    original_node_indices: HashMap<NodeId, usize>,
}

pub fn solve(model: &BridgeModel, options: SolveOptions) -> Result<AnalysisResult, SolveError> {
    solve_with(model, options, &DensePivotSolver)
}

pub fn solve_with(
    model: &BridgeModel,
    options: SolveOptions,
    linear_solver: &dyn LinearSystemSolver,
) -> Result<AnalysisResult, SolveError> {
    model
        .validate()
        .map_err(|error| SolveError::InvalidModel(error.to_string()))?;
    let load_case = model.active_load_case();
    let mesh = build_mesh(model)?;
    let degree_count = mesh.coordinates_m.len() * 2;
    let maximum_degrees = options.max_degrees_of_freedom.max(4);
    if degree_count > maximum_degrees {
        return Err(SolveError::ModelTooLarge {
            degrees_of_freedom: degree_count,
            maximum: maximum_degrees,
        });
    }
    let mut stiffness = vec![vec![0.0; degree_count]; degree_count];
    let mut loads = vec![0.0; degree_count];

    for element in &mesh.elements {
        let length = element.length_m();
        if !length.is_finite()
            || length <= COORDINATE_TOLERANCE_M
            || !element.flexural_rigidity_nm2.is_finite()
            || element.flexural_rigidity_nm2 <= 0.0
        {
            return Err(SolveError::InvalidElement(element.parent));
        }
        let local_stiffness = beam_stiffness(element.flexural_rigidity_nm2, length);
        let equivalent_load = trapezoidal_equivalent_loads(
            element.start_load_down_n_per_m,
            element.end_load_down_n_per_m,
            length,
        );
        let map = [
            element.start_index * 2,
            element.start_index * 2 + 1,
            element.end_index * 2,
            element.end_index * 2 + 1,
        ];
        for local_row in 0..4 {
            loads[map[local_row]] += equivalent_load[local_row];
            for local_column in 0..4 {
                stiffness[map[local_row]][map[local_column]] +=
                    local_stiffness[local_row][local_column];
            }
        }
    }

    for point_load in &load_case.point_loads {
        let element = model
            .element(point_load.element)
            .ok_or(SolveError::MissingLoadCase)?;
        let start = model
            .node(element.start)
            .ok_or(SolveError::MissingLoadCase)?;
        let end = model.node(element.end).ok_or(SolveError::MissingLoadCase)?;
        let x_m = start.position.x_m
            + point_load.relative_position * (end.position.x_m - start.position.x_m);
        let index = coordinate_index(&mesh.coordinates_m, x_m)
            .ok_or(SolveError::InvalidElement(element.id))?;
        loads[index * 2] -= point_load.force_down_n;
        loads[index * 2 + 1] += point_load.moment_nm;
    }

    let mut restrained = vec![false; degree_count];
    for support in model.supports() {
        let index = *mesh
            .original_node_indices
            .get(&support.node)
            .ok_or_else(|| SolveError::InvalidModel("支座节点未进入分析网格".to_string()))?;
        restrained[index * 2] |= support.vertical;
        restrained[index * 2 + 1] |= support.rotation;
    }
    let free = restrained
        .iter()
        .enumerate()
        .filter_map(|(index, is_restrained)| (!is_restrained).then_some(index))
        .collect::<Vec<_>>();

    let reduced_stiffness = free
        .iter()
        .map(|row| free.iter().map(|column| stiffness[*row][*column]).collect())
        .collect::<Vec<Vec<_>>>();
    let reduced_loads = free.iter().map(|index| loads[*index]).collect::<Vec<_>>();
    let reduced_displacements = linear_solver.solve(reduced_stiffness, reduced_loads)?;
    let mut displacements = vec![0.0; degree_count];
    for (local_index, global_index) in free.iter().copied().enumerate() {
        displacements[global_index] = reduced_displacements[local_index];
    }

    let reactions = stiffness
        .iter()
        .enumerate()
        .map(|(row, values)| {
            values
                .iter()
                .zip(&displacements)
                .map(|(coefficient, displacement)| coefficient * displacement)
                .sum::<f64>()
                - loads[row]
        })
        .collect::<Vec<_>>();

    let node_results = model
        .nodes()
        .iter()
        .map(|node| {
            let index = mesh.original_node_indices[&node.id];
            NodeResult {
                node: node.id,
                x_m: node.position.x_m,
                displacement_m: displacements[index * 2],
                rotation_rad: displacements[index * 2 + 1],
                reaction_vertical_n: if restrained[index * 2] {
                    reactions[index * 2]
                } else {
                    0.0
                },
                reaction_moment_nm: if restrained[index * 2 + 1] {
                    reactions[index * 2 + 1]
                } else {
                    0.0
                },
            }
        })
        .collect::<Vec<_>>();

    let element_results = mesh
        .elements
        .iter()
        .map(|element| {
            let length = element.length_m();
            let local_stiffness = beam_stiffness(element.flexural_rigidity_nm2, length);
            let equivalent_load = trapezoidal_equivalent_loads(
                element.start_load_down_n_per_m,
                element.end_load_down_n_per_m,
                length,
            );
            let map = [
                element.start_index * 2,
                element.start_index * 2 + 1,
                element.end_index * 2,
                element.end_index * 2 + 1,
            ];
            let end_forces = std::array::from_fn::<_, 4, _>(|row| {
                (0..4)
                    .map(|column| local_stiffness[row][column] * displacements[map[column]])
                    .sum::<f64>()
                    - equivalent_load[row]
            });
            ElementResult {
                parent: element.parent,
                start_x_m: element.start_x_m,
                end_x_m: element.end_x_m,
                start_shear_n: end_forces[0],
                start_moment_nm: end_forces[1],
                end_shear_n: end_forces[2],
                end_moment_nm: end_forces[3],
            }
        })
        .collect::<Vec<_>>();

    let sample_count = options.samples_per_segment.clamp(4, 256);
    let mut sample_x = mesh
        .elements
        .iter()
        .flat_map(|element| {
            (0..=sample_count).map(move |index| {
                element.start_x_m + element.length_m() * index as f64 / sample_count as f64
            })
        })
        .collect::<Vec<_>>();
    let first_x = mesh.coordinates_m[0];
    let last_x = mesh.coordinates_m[mesh.coordinates_m.len() - 1];
    let event_offset = (last_x - first_x).max(1.0) * 1.0e-8;
    for coordinate in &mesh.coordinates_m {
        if *coordinate > first_x + event_offset {
            sample_x.push(*coordinate - event_offset);
        }
        sample_x.push(*coordinate);
        if *coordinate < last_x - event_offset {
            sample_x.push(*coordinate + event_offset);
        }
    }
    sample_x.sort_by(f64::total_cmp);
    sample_x.dedup_by(|left, right| (*left - *right).abs() <= COORDINATE_TOLERANCE_M);

    let diagram = sample_x
        .into_iter()
        .map(|x_m| {
            let (shear_n, moment_nm) = internal_force_at(model, &node_results, load_case.id, x_m);
            let (displacement_m, rotation_rad) = displacement_at(&mesh, &displacements, x_m);
            DiagramPoint {
                x_m,
                shear_n,
                moment_nm,
                displacement_m,
                rotation_rad,
            }
        })
        .collect::<Vec<_>>();

    let max_abs_shear_n = diagram
        .iter()
        .map(|sample| sample.shear_n.abs())
        .fold(0.0, f64::max);
    let max_abs_moment_nm = diagram
        .iter()
        .map(|sample| sample.moment_nm.abs())
        .fold(0.0, f64::max);
    let max_abs_displacement_m = diagram
        .iter()
        .map(|sample| sample.displacement_m.abs())
        .fold(0.0, f64::max);
    let total_reaction_n = node_results
        .iter()
        .map(|node| node.reaction_vertical_n)
        .sum::<f64>();
    let total_point_load_n = load_case
        .point_loads
        .iter()
        .map(|load| load.force_down_n)
        .sum::<f64>();
    let total_distributed_load_n = load_case
        .distributed_loads
        .iter()
        .map(|load| {
            let element = model
                .element(load.element)
                .expect("validated model contains load element");
            let start = model
                .node(element.start)
                .expect("validated model contains element node");
            let end = model
                .node(element.end)
                .expect("validated model contains element node");
            (load.start_down_n_per_m + load.end_down_n_per_m)
                * 0.5
                * (end.position.x_m - start.position.x_m)
        })
        .sum::<f64>();
    let moment_origin_m = model.nodes()[0].position.x_m;
    let reaction_moment_sum_nm = node_results
        .iter()
        .map(|node| {
            node.reaction_vertical_n * (node.x_m - moment_origin_m) + node.reaction_moment_nm
        })
        .sum::<f64>();
    let point_load_moment_sum_nm = load_case
        .point_loads
        .iter()
        .map(|load| {
            let element = model
                .element(load.element)
                .expect("validated model contains load element");
            let start = model
                .node(element.start)
                .expect("validated model contains element node");
            let end = model
                .node(element.end)
                .expect("validated model contains element node");
            let x_m = start.position.x_m
                + load.relative_position * (end.position.x_m - start.position.x_m);
            load.force_down_n * (x_m - moment_origin_m) - load.moment_nm
        })
        .sum::<f64>();
    let distributed_load_moment_sum_nm = load_case
        .distributed_loads
        .iter()
        .map(|load| {
            let element = model
                .element(load.element)
                .expect("validated model contains load element");
            let start_x_m = model
                .node(element.start)
                .expect("validated model contains element node")
                .position
                .x_m;
            let end_x_m = model
                .node(element.end)
                .expect("validated model contains element node")
                .position
                .x_m;
            let length_m = end_x_m - start_x_m;
            let slope = (load.end_down_n_per_m - load.start_down_n_per_m) / length_m;
            let resultant_n = load.start_down_n_per_m * length_m + 0.5 * slope * length_m.powi(2);
            let first_moment_from_start_nm =
                0.5 * load.start_down_n_per_m * length_m.powi(2) + slope * length_m.powi(3) / 3.0;
            (start_x_m - moment_origin_m) * resultant_n + first_moment_from_start_nm
        })
        .sum::<f64>();
    let restrained_count = model
        .supports()
        .iter()
        .map(|support| usize::from(support.vertical) + usize::from(support.rotation))
        .sum::<usize>();

    Ok(AnalysisResult {
        load_case: load_case.id,
        total_length_m: model.total_length_m(),
        node_results,
        element_results,
        diagram,
        max_abs_shear_n,
        max_abs_moment_nm,
        max_abs_displacement_m,
        equilibrium_residual_n: total_reaction_n - total_point_load_n - total_distributed_load_n,
        equilibrium_moment_residual_nm: reaction_moment_sum_nm
            - point_load_moment_sum_nm
            - distributed_load_moment_sum_nm,
        mesh_node_count: mesh.coordinates_m.len(),
        static_indeterminacy: restrained_count.saturating_sub(2),
    })
}

fn build_mesh(model: &BridgeModel) -> Result<AnalysisMesh, SolveError> {
    let load_case = model.active_load_case();
    let mut coordinates_m = model
        .nodes()
        .iter()
        .map(|node| node.position.x_m)
        .collect::<Vec<_>>();
    for load in &load_case.point_loads {
        let element = model
            .element(load.element)
            .ok_or(SolveError::MissingLoadCase)?;
        let start = model
            .node(element.start)
            .ok_or(SolveError::MissingLoadCase)?;
        let end = model.node(element.end).ok_or(SolveError::MissingLoadCase)?;
        coordinates_m.push(
            start.position.x_m + load.relative_position * (end.position.x_m - start.position.x_m),
        );
    }
    coordinates_m.sort_by(f64::total_cmp);
    coordinates_m.dedup_by(|left, right| (*left - *right).abs() <= COORDINATE_TOLERANCE_M);

    let original_node_indices = model
        .nodes()
        .iter()
        .map(|node| {
            coordinate_index(&coordinates_m, node.position.x_m)
                .map(|index| (node.id, index))
                .ok_or_else(|| SolveError::InvalidModel("节点无法映射到分析网格".to_string()))
        })
        .collect::<Result<HashMap<_, _>, _>>()?;

    let mut elements = Vec::with_capacity(coordinates_m.len().saturating_sub(1));
    for (start_index, coordinates) in coordinates_m.windows(2).enumerate() {
        let start_x_m = coordinates[0];
        let end_x_m = coordinates[1];
        let midpoint = (start_x_m + end_x_m) * 0.5;
        let parent = model
            .elements()
            .iter()
            .find(|element| {
                let start = model
                    .node(element.start)
                    .expect("validated element has start node");
                let end = model
                    .node(element.end)
                    .expect("validated element has end node");
                midpoint >= start.position.x_m - COORDINATE_TOLERANCE_M
                    && midpoint <= end.position.x_m + COORDINATE_TOLERANCE_M
            })
            .ok_or_else(|| SolveError::InvalidModel("分析网格存在未归属区段".to_string()))?;
        let material = model
            .material(parent.material)
            .ok_or_else(|| SolveError::InvalidModel("单元材料不存在".to_string()))?;
        let section = model
            .section(parent.section)
            .ok_or_else(|| SolveError::InvalidModel("单元截面不存在".to_string()))?;
        let parent_start = model
            .node(parent.start)
            .expect("validated element has start node")
            .position
            .x_m;
        let parent_end = model
            .node(parent.end)
            .expect("validated element has end node")
            .position
            .x_m;
        let parent_length = parent_end - parent_start;
        let start_ratio = (start_x_m - parent_start) / parent_length;
        let end_ratio = (end_x_m - parent_start) / parent_length;
        let (start_load, end_load) = load_case
            .distributed_loads
            .iter()
            .filter(|load| load.element == parent.id)
            .fold((0.0, 0.0), |(start_sum, end_sum), load| {
                let slope = load.end_down_n_per_m - load.start_down_n_per_m;
                (
                    start_sum + load.start_down_n_per_m + slope * start_ratio,
                    end_sum + load.start_down_n_per_m + slope * end_ratio,
                )
            });
        elements.push(MeshElement {
            parent: parent.id,
            start_index,
            end_index: start_index + 1,
            start_x_m,
            end_x_m,
            flexural_rigidity_nm2: material.elastic_modulus_pa * section.inertia_m4,
            start_load_down_n_per_m: start_load,
            end_load_down_n_per_m: end_load,
        });
    }

    Ok(AnalysisMesh {
        coordinates_m,
        elements,
        original_node_indices,
    })
}

fn coordinate_index(coordinates: &[f64], x_m: f64) -> Option<usize> {
    coordinates
        .iter()
        .position(|coordinate| (*coordinate - x_m).abs() <= COORDINATE_TOLERANCE_M)
}

fn beam_stiffness(flexural_rigidity_nm2: f64, length_m: f64) -> [[f64; 4]; 4] {
    let factor = flexural_rigidity_nm2 / length_m.powi(3);
    let length = length_m;
    [
        [
            12.0 * factor,
            6.0 * length * factor,
            -12.0 * factor,
            6.0 * length * factor,
        ],
        [
            6.0 * length * factor,
            4.0 * length * length * factor,
            -6.0 * length * factor,
            2.0 * length * length * factor,
        ],
        [
            -12.0 * factor,
            -6.0 * length * factor,
            12.0 * factor,
            -6.0 * length * factor,
        ],
        [
            6.0 * length * factor,
            2.0 * length * length * factor,
            -6.0 * length * factor,
            4.0 * length * length * factor,
        ],
    ]
}

fn trapezoidal_equivalent_loads(
    start_down_n_per_m: f64,
    end_down_n_per_m: f64,
    length_m: f64,
) -> [f64; 4] {
    let left_vertical = -length_m * (7.0 * start_down_n_per_m + 3.0 * end_down_n_per_m) / 20.0;
    let left_moment =
        -length_m.powi(2) * (3.0 * start_down_n_per_m + 2.0 * end_down_n_per_m) / 60.0;
    let right_vertical = -length_m * (3.0 * start_down_n_per_m + 7.0 * end_down_n_per_m) / 20.0;
    let right_moment =
        length_m.powi(2) * (2.0 * start_down_n_per_m + 3.0 * end_down_n_per_m) / 60.0;
    [left_vertical, left_moment, right_vertical, right_moment]
}

fn internal_force_at(
    model: &BridgeModel,
    node_results: &[NodeResult],
    load_case_id: LoadCaseId,
    x_m: f64,
) -> (f64, f64) {
    let tolerance = model.total_length_m().max(1.0) * 1.0e-10;
    let mut shear_n = 0.0;
    let mut moment_nm = 0.0;

    for node in node_results {
        if node.x_m <= x_m + tolerance {
            shear_n += node.reaction_vertical_n;
            moment_nm += node.reaction_vertical_n * (x_m - node.x_m);
            moment_nm -= node.reaction_moment_nm;
        }
    }

    let load_case = model
        .load_cases()
        .iter()
        .find(|load_case| load_case.id == load_case_id)
        .expect("result load case belongs to validated model");
    for load in &load_case.point_loads {
        let element = model
            .element(load.element)
            .expect("validated point load has element");
        let start = model
            .node(element.start)
            .expect("validated element has start");
        let end = model.node(element.end).expect("validated element has end");
        let load_x =
            start.position.x_m + load.relative_position * (end.position.x_m - start.position.x_m);
        if load_x <= x_m + tolerance {
            shear_n -= load.force_down_n;
            moment_nm -= load.force_down_n * (x_m - load_x);
            moment_nm -= load.moment_nm;
        }
    }

    for load in &load_case.distributed_loads {
        let element = model
            .element(load.element)
            .expect("validated distributed load has element");
        let element_start = model
            .node(element.start)
            .expect("validated element has start")
            .position
            .x_m;
        let element_end = model
            .node(element.end)
            .expect("validated element has end")
            .position
            .x_m;
        if x_m <= element_start {
            continue;
        }
        let length = element_end - element_start;
        let loaded_length = (x_m.min(element_end) - element_start).clamp(0.0, length);
        let slope = (load.end_down_n_per_m - load.start_down_n_per_m) / length;
        let resultant =
            load.start_down_n_per_m * loaded_length + 0.5 * slope * loaded_length.powi(2);
        let first_moment_from_start = 0.5 * load.start_down_n_per_m * loaded_length.powi(2)
            + slope * loaded_length.powi(3) / 3.0;
        shear_n -= resultant;
        moment_nm -= (x_m - element_start) * resultant - first_moment_from_start;
    }
    (shear_n, moment_nm)
}

fn displacement_at(mesh: &AnalysisMesh, displacements: &[f64], x_m: f64) -> (f64, f64) {
    let element = mesh
        .elements
        .iter()
        .find(|element| {
            x_m >= element.start_x_m - COORDINATE_TOLERANCE_M
                && x_m <= element.end_x_m + COORDINATE_TOLERANCE_M
        })
        .unwrap_or_else(|| {
            mesh.elements
                .last()
                .expect("validated bridge mesh contains an element")
        });
    let length = element.length_m();
    let ratio = ((x_m - element.start_x_m) / length).clamp(0.0, 1.0);
    let n1 = 1.0 - 3.0 * ratio.powi(2) + 2.0 * ratio.powi(3);
    let n2 = length * (ratio - 2.0 * ratio.powi(2) + ratio.powi(3));
    let n3 = 3.0 * ratio.powi(2) - 2.0 * ratio.powi(3);
    let n4 = length * (-ratio.powi(2) + ratio.powi(3));
    let displacement = n1 * displacements[element.start_index * 2]
        + n2 * displacements[element.start_index * 2 + 1]
        + n3 * displacements[element.end_index * 2]
        + n4 * displacements[element.end_index * 2 + 1];

    let dn1_dx = (-6.0 * ratio + 6.0 * ratio.powi(2)) / length;
    let dn2_dx = 1.0 - 4.0 * ratio + 3.0 * ratio.powi(2);
    let dn3_dx = (6.0 * ratio - 6.0 * ratio.powi(2)) / length;
    let dn4_dx = -2.0 * ratio + 3.0 * ratio.powi(2);
    let rotation = dn1_dx * displacements[element.start_index * 2]
        + dn2_dx * displacements[element.start_index * 2 + 1]
        + dn3_dx * displacements[element.end_index * 2]
        + dn4_dx * displacements[element.end_index * 2 + 1];
    (displacement, rotation)
}

#[cfg(test)]
mod tests {
    use bridge_core::{BridgeModel, DistributedLoad, LoadId, ModelParts};

    use super::*;

    fn simple_beam() -> BridgeModel {
        let mut model = BridgeModel::continuous_beam("简支梁", &[10.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid beam");
        model
            .set_primary_point_load(5.0, 100_000.0)
            .expect("valid point load");
        model
    }

    #[test]
    fn simply_supported_midspan_load_matches_closed_form() {
        let model = simple_beam();
        let result = solve(&model, SolveOptions::default()).expect("stable beam");
        let left = result.node(model.nodes()[0].id).expect("left result");
        let right = result.node(model.nodes()[1].id).expect("right result");
        let expected_displacement = 100_000.0 * 10.0_f64.powi(3) / (48.0 * 200.0e9 * 8.0e-6);

        assert!((left.reaction_vertical_n - 50_000.0).abs() < 1.0e-5);
        assert!((right.reaction_vertical_n - 50_000.0).abs() < 1.0e-5);
        assert!((result.max_abs_moment_nm - 250_000.0).abs() < 1.0e-4);
        assert!(
            (result.max_abs_displacement_m - expected_displacement).abs()
                < expected_displacement * 1.0e-10
        );
        assert!(result.equilibrium_residual_n.abs() < 1.0e-6);
        assert!(result.equilibrium_moment_residual_nm.abs() < 1.0e-5);
    }

    #[test]
    fn cantilever_tip_load_matches_closed_form() {
        let model = BridgeModel::continuous_beam("悬臂梁", &[4.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid beam");
        let mut parts = model.parts().clone();
        parts.supports.truncate(1);
        parts.supports[0].rotation = true;
        let mut model = BridgeModel::from_parts(parts).expect("valid cantilever");
        model
            .set_primary_point_load(4.0, 20_000.0)
            .expect("valid point load");

        let result = solve(&model, SolveOptions::default()).expect("stable cantilever");
        let root = result.node(model.nodes()[0].id).expect("root result");
        let expected_displacement = 20_000.0 * 4.0_f64.powi(3) / (3.0 * 200.0e9 * 8.0e-6);

        assert!((root.reaction_vertical_n - 20_000.0).abs() < 1.0e-6);
        assert!((root.reaction_moment_nm - 80_000.0).abs() < 1.0e-6);
        assert!(
            (result.max_abs_displacement_m - expected_displacement).abs()
                < expected_displacement * 1.0e-10
        );
        assert!(result.equilibrium_moment_residual_nm.abs() < 1.0e-5);
    }

    #[test]
    fn uniform_load_preserves_vertical_equilibrium() {
        let model = BridgeModel::continuous_beam("均布荷载", &[8.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid beam");
        let mut parts: ModelParts = model.parts().clone();
        parts.load_cases[0].distributed_loads.push(DistributedLoad {
            id: LoadId::new(100),
            element: parts.elements[0].id,
            start_down_n_per_m: 10_000.0,
            end_down_n_per_m: 10_000.0,
        });
        let model = BridgeModel::from_parts(parts).expect("valid loaded beam");

        let result = solve(&model, SolveOptions::default()).expect("stable beam");

        assert!(result.equilibrium_residual_n.abs() < 1.0e-6);
        assert!(result.equilibrium_moment_residual_nm.abs() < 1.0e-5);
        assert!((result.max_abs_moment_nm - 80_000.0).abs() < 1.0e-4);
    }

    #[test]
    fn symmetric_two_span_beam_matches_classical_reactions() {
        let model =
            BridgeModel::continuous_beam("两跨连续梁", &[10.0, 10.0], 200.0e9, 0.12, 8.0e-6)
                .expect("valid beam");
        let mut parts = model.parts().clone();
        for (index, element) in parts.elements.iter().enumerate() {
            parts.load_cases[0].distributed_loads.push(DistributedLoad {
                id: LoadId::new(100 + index as u64),
                element: element.id,
                start_down_n_per_m: 10_000.0,
                end_down_n_per_m: 10_000.0,
            });
        }
        let model = BridgeModel::from_parts(parts).expect("valid two-span beam");

        let result = solve(&model, SolveOptions::default()).expect("stable two-span beam");
        let reactions = model
            .nodes()
            .iter()
            .map(|node| {
                result
                    .node(node.id)
                    .expect("node result")
                    .reaction_vertical_n
            })
            .collect::<Vec<_>>();

        assert!((reactions[0] - 37_500.0).abs() < 1.0e-5);
        assert!((reactions[1] - 125_000.0).abs() < 1.0e-5);
        assert!((reactions[2] - 37_500.0).abs() < 1.0e-5);
        assert!(result.equilibrium_residual_n.abs() < 1.0e-6);
        assert!(result.equilibrium_moment_residual_nm.abs() < 1.0e-5);
    }

    #[test]
    fn fully_restrained_unloaded_beam_is_a_valid_system() {
        let model = BridgeModel::continuous_beam("全约束梁", &[4.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid beam");
        let mut parts = model.parts().clone();
        for support in &mut parts.supports {
            support.rotation = true;
        }
        let model = BridgeModel::from_parts(parts).expect("valid fixed beam");

        let result = solve(&model, SolveOptions::default()).expect("zero-free-DOF system");

        assert_eq!(result.max_abs_displacement_m, 0.0);
        assert_eq!(result.equilibrium_residual_n, 0.0);
    }

    #[test]
    fn dense_backend_guardrail_rejects_oversized_model_before_allocation() {
        let model = BridgeModel::continuous_beam("大模型", &[1.0, 1.0, 1.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid beam");
        let options = SolveOptions {
            max_degrees_of_freedom: 4,
            ..SolveOptions::default()
        };

        assert_eq!(
            solve(&model, options),
            Err(SolveError::ModelTooLarge {
                degrees_of_freedom: 8,
                maximum: 4
            })
        );
    }

    #[test]
    fn mechanism_is_reported_instead_of_panicking() {
        let model = simple_beam();
        let mut parts = model.parts().clone();
        parts.supports.clear();
        let model =
            BridgeModel::from_parts(parts).expect("unrestrained model is structurally valid");

        assert_eq!(
            solve(&model, SolveOptions::default()),
            Err(SolveError::SingularSystem)
        );
    }
}
