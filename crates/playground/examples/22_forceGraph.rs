//! BridgeLab 多跨连续梁有限元程序的兼容启动入口。
//!
//! 完整实现按职责维护在 `bridge_app`、`bridge_core`、`bridge_solver`、
//! `bridge_io` 与 `bridge_validation` 中，避免单个示例文件再次回退或分叉。
//!
//! 运行：
//! `cargo run -p playground --example 22_forceGraph`
//!
//! 也可以在命令后传入工程文件，启动后直接打开：
//! `cargo run -p playground --example 22_forceGraph -- demo.bridge.json`

fn main() {
    bridge_app::run();
}
