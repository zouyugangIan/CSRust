//! BridgeLab compatibility launcher.
//!
//! The maintained implementation lives in `crates/bridge_app/src/lib.rs`.
//! Run it directly with `cargo run -p bridge-app`, or keep using:
//! `cargo run -p playground --example 24_forceGraph`.

fn main() {
    bridge_app::run();
}
