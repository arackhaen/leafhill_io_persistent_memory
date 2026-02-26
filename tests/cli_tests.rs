use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_leafhill-persistent-memory"))
}

#[test]
fn test_cli_version() {
    let output = binary().arg("--version").output().expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("leafhill-persistent-memory"));
}

#[test]
fn test_cli_help() {
    let output = binary().arg("--help").output().expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("leafhill.io Persistent Claude Memory"));
}

#[test]
fn test_cli_invalid_subcommand() {
    let output = binary().arg("nonexistent-command").output().expect("failed to run");
    assert!(!output.status.success());
}
