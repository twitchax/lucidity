//! Helpers shared by the end-to-end tests.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

pub fn lunatic() -> PathBuf {
    let bin = std::env::var_os("LUNATIC_BIN").map_or_else(|| workspace_root().join("../lunatic/target/release/lunatic"), PathBuf::from);
    assert!(bin.is_file(), "no lunatic runtime at {}: run `cargo build --release` in ../lunatic, or set LUNATIC_BIN", bin.display());

    bin
}

/// Builds an example of the `lucidity` crate for wasm, in its own target directory so it doesn't wait on this build.
pub fn example(name: &str) -> PathBuf {
    let target_dir = workspace_root().join("target/e2e");
    let status = Command::new(env!("CARGO"))
        .current_dir(workspace_root())
        .args(["build", "--target", "wasm32-wasip1", "-p", "lucidity", "--example", name, "--target-dir"])
        .arg(&target_dir)
        .status()
        .unwrap();
    assert!(status.success(), "building example `{name}` failed");

    target_dir.join(format!("wasm32-wasip1/debug/examples/{name}.wasm"))
}
