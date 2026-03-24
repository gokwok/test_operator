use std::{path::PathBuf, process::Command};

use serde_json::Value;

#[test]
fn operator_mcp_package_exports_library_only() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate lives under workspace root")
        .to_path_buf();

    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .expect("cargo metadata should run");

    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata should emit JSON");
    let package = metadata["packages"]
        .as_array()
        .and_then(|packages| {
            packages
                .iter()
                .find(|package| package["name"].as_str() == Some("operator-mcp"))
        })
        .expect("operator-mcp package should exist");

    let targets = package["targets"]
        .as_array()
        .expect("package targets should be an array");

    assert!(
        targets
            .iter()
            .any(|target| target["kind"].as_array().is_some_and(|kind| {
                kind.iter().any(|entry| entry.as_str() == Some("lib"))
            })),
        "operator-mcp should keep exporting a library target"
    );
    assert!(
        targets.iter().all(|target| {
            !target["kind"].as_array().is_some_and(|kind| {
                kind.iter().any(|entry| entry.as_str() == Some("bin"))
            })
        }),
        "operator-mcp should not expose a binary target"
    );
}
