use std::process::Command;

/// Running `sendit` outside any git repository must refuse with exit code 2.
/// This exercises the real binary and the process-spawn path without needing
/// `gh` authentication (the repo check runs first).
#[test]
fn not_a_repo_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_sendit"))
        .current_dir(dir.path())
        .args(["--yes", "--title", "x"])
        .output()
        .expect("failed to run sendit");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not inside a git repository"),
        "unexpected stderr: {stderr}"
    );
}
