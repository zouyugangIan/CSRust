//! Independent result checks and closed-form benchmark cases.

use bridge_core::BridgeModel;
use bridge_solver::AnalysisResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindingLevel {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub level: FindingLevel,
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidationReport {
    pub findings: Vec<Finding>,
}

impl ValidationReport {
    #[must_use]
    pub fn passed(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|finding| finding.level == FindingLevel::Error)
    }

    #[must_use]
    pub fn summary(&self) -> String {
        if self.findings.is_empty() {
            "平衡与边界检查通过".to_string()
        } else {
            let errors = self
                .findings
                .iter()
                .filter(|finding| finding.level == FindingLevel::Error)
                .count();
            let warnings = self.findings.len() - errors;
            format!("{errors} 个错误，{warnings} 个警告")
        }
    }
}

#[must_use]
pub fn validate_result(model: &BridgeModel, result: &AnalysisResult) -> ValidationReport {
    let mut report = ValidationReport::default();
    if let Err(error) = model.validate() {
        report.findings.push(Finding {
            level: FindingLevel::Error,
            code: "INVALID_MODEL",
            message: error.to_string(),
        });
        return report;
    }
    if result.load_case != model.active_load_case_id() {
        report.findings.push(Finding {
            level: FindingLevel::Error,
            code: "RESULT_LOAD_CASE",
            message: "结果与当前荷载工况不一致".to_string(),
        });
    }
    if result.diagram.is_empty() {
        report.findings.push(Finding {
            level: FindingLevel::Error,
            code: "EMPTY_DIAGRAM",
            message: "求解结果没有图表采样点".to_string(),
        });
    }
    if result.diagram.iter().any(|point| {
        !point.x_m.is_finite()
            || !point.shear_n.is_finite()
            || !point.moment_nm.is_finite()
            || !point.displacement_m.is_finite()
            || !point.rotation_rad.is_finite()
    }) {
        report.findings.push(Finding {
            level: FindingLevel::Error,
            code: "NON_FINITE_RESULT",
            message: "结果包含 NaN 或无穷大".to_string(),
        });
    }

    let total_load_n = model
        .active_load_case()
        .point_loads
        .iter()
        .map(|load| load.force_down_n.abs())
        .sum::<f64>()
        + model
            .active_load_case()
            .distributed_loads
            .iter()
            .map(|load| {
                let element = model
                    .element(load.element)
                    .expect("validated load references an element");
                let start = model
                    .node(element.start)
                    .expect("validated element references start node");
                let end = model
                    .node(element.end)
                    .expect("validated element references end node");
                (load.start_down_n_per_m.abs() + load.end_down_n_per_m.abs())
                    * 0.5
                    * (end.position.x_m - start.position.x_m)
            })
            .sum::<f64>();
    let equilibrium_tolerance_n = (total_load_n * 1.0e-8).max(1.0e-5);
    if result.equilibrium_residual_n.abs() > equilibrium_tolerance_n {
        report.findings.push(Finding {
            level: FindingLevel::Error,
            code: "VERTICAL_EQUILIBRIUM",
            message: format!(
                "竖向平衡残差 {:.3e} N 超过容差 {:.3e} N",
                result.equilibrium_residual_n, equilibrium_tolerance_n
            ),
        });
    }
    let moment_scale_nm = total_load_n * model.total_length_m()
        + model
            .active_load_case()
            .point_loads
            .iter()
            .map(|load| load.moment_nm.abs())
            .sum::<f64>();
    let moment_tolerance_nm = (moment_scale_nm * 1.0e-8).max(1.0e-4);
    if result.equilibrium_moment_residual_nm.abs() > moment_tolerance_nm {
        report.findings.push(Finding {
            level: FindingLevel::Error,
            code: "MOMENT_EQUILIBRIUM",
            message: format!(
                "力矩平衡残差 {:.3e} N·m 超过容差 {:.3e} N·m",
                result.equilibrium_moment_residual_nm, moment_tolerance_nm
            ),
        });
    }

    let displacement_scale = result.max_abs_displacement_m.max(1.0);
    let boundary_tolerance_m = displacement_scale * 1.0e-10;
    for support in model.supports() {
        let Some(node_result) = result.node(support.node) else {
            report.findings.push(Finding {
                level: FindingLevel::Error,
                code: "MISSING_NODE_RESULT",
                message: format!("节点 {} 缺少分析结果", support.node.get()),
            });
            continue;
        };
        if support.vertical && node_result.displacement_m.abs() > boundary_tolerance_m {
            report.findings.push(Finding {
                level: FindingLevel::Error,
                code: "VERTICAL_BOUNDARY",
                message: format!(
                    "节点 {} 的约束位移 {:.3e} m 超过容差",
                    support.node.get(),
                    node_result.displacement_m
                ),
            });
        }
        if support.rotation && node_result.rotation_rad.abs() > 1.0e-10 {
            report.findings.push(Finding {
                level: FindingLevel::Error,
                code: "ROTATION_BOUNDARY",
                message: format!(
                    "节点 {} 的约束转角 {:.3e} rad 超过容差",
                    support.node.get(),
                    node_result.rotation_rad
                ),
            });
        }
    }

    if result.mesh_node_count > 5_000 {
        report.findings.push(Finding {
            level: FindingLevel::Warning,
            code: "DENSE_SOLVER_SCALE",
            message: "分析网格超过 5000 个节点，建议切换稀疏矩阵后端".to_string(),
        });
    }
    report
}

#[cfg(test)]
mod tests {
    use bridge_solver::{SolveOptions, solve};

    use super::*;

    #[test]
    fn verified_reference_case_has_no_findings() {
        let mut model = BridgeModel::continuous_beam("基准梁", &[10.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid model");
        model
            .set_primary_point_load(5.0, 100_000.0)
            .expect("valid load");
        let result = solve(&model, SolveOptions::default()).expect("stable model");

        let report = validate_result(&model, &result);

        assert!(report.passed());
        assert_eq!(report.summary(), "平衡与边界检查通过");
        assert!(report.findings.is_empty());
    }

    #[test]
    fn tampered_result_is_rejected() {
        let mut model = BridgeModel::continuous_beam("基准梁", &[10.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid model");
        model
            .set_primary_point_load(5.0, 100_000.0)
            .expect("valid load");
        let mut result = solve(&model, SolveOptions::default()).expect("stable model");
        result.equilibrium_residual_n = 1_000.0;

        let report = validate_result(&model, &result);

        assert!(!report.passed());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "VERTICAL_EQUILIBRIUM")
        );
    }

    #[test]
    fn moment_equilibrium_is_checked_independently() {
        let mut model = BridgeModel::continuous_beam("基准梁", &[10.0], 200.0e9, 0.12, 8.0e-6)
            .expect("valid model");
        model
            .set_primary_point_load(5.0, 100_000.0)
            .expect("valid load");
        let mut result = solve(&model, SolveOptions::default()).expect("stable model");
        result.equilibrium_moment_residual_nm = 5_000.0;

        let report = validate_result(&model, &result);

        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "MOMENT_EQUILIBRIUM")
        );
    }
}
