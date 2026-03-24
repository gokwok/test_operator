use std::{path::PathBuf, process::Command};

#[test]
fn local_run_help_describes_the_developer_harness_surface() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate lives under workspace root")
        .to_path_buf();

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "operator-agent",
            "--example",
            "local_run",
            "--",
            "--help",
        ])
        .current_dir(workspace_root)
        .output()
        .expect("cargo run --example local_run -- --help should run");

    assert!(
        output.status.success(),
        "help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("developer-only local agent harness"),
        "help output should mark the example as developer-only: {stdout}"
    );
    assert!(
        stdout.contains("--task"),
        "help output should expose --task: {stdout}"
    );
    assert!(
        stdout.contains("--target"),
        "help output should expose --target: {stdout}"
    );
    assert!(
        stdout.contains("--model"),
        "help output should expose --model: {stdout}"
    );
}
