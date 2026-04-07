use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("deepwiki-dl").unwrap()
}

#[test]
fn test_help() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("deepwiki-dl"))
        .stdout(predicate::str::contains("Download DeepWiki documentation"));
}

#[test]
fn test_version() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("deepwiki-dl"));
}

#[test]
fn test_no_args_shows_help() {
    // No arguments should show help (exit 0 from clap)
    cmd()
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_pull_help() {
    cmd()
        .args(["pull", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pull documentation from DeepWiki"));
}

#[test]
fn test_list_help() {
    cmd()
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List available sections"));
}

#[test]
fn test_invalid_repo_format() {
    cmd()
        .arg("not-a-valid-repo")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Expected 'owner/repo'"));
}

#[test]
fn test_mermaid_without_output_fails() {
    // --mermaid without -o should fail with a helpful message
    // We use a fake endpoint to avoid hitting the real API
    cmd()
        .env("DEEPWIKI_DL_MCP_ENDPOINT", "http://localhost:1/fake")
        .args(["pull", "--mermaid", "svg", "owner/repo"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--mermaid requires -o"));
}

#[test]
fn test_pull_subcommand_invalid_repo() {
    cmd()
        .args(["pull", "badformat"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Expected 'owner/repo'"));
}

#[test]
fn test_list_subcommand_invalid_repo() {
    cmd()
        .args(["list", "badformat"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Expected 'owner/repo'"));
}

// Integration tests that hit the real DeepWiki API — marked #[ignore]
// Run with: cargo test --test cli -- --ignored

#[test]
#[ignore]
fn test_list_real_api() {
    cmd()
        .args(["list", "anthropics/claude-code"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Overview"));
}

#[test]
#[ignore]
fn test_list_json_real_api() {
    cmd()
        .args(["list", "--json", "anthropics/claude-code"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"repo\""))
        .stdout(predicate::str::contains("\"pages\""));
}

#[test]
#[ignore]
fn test_pull_stdout_real_api() {
    cmd()
        .args(["anthropics/claude-code", "--pages", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Claude Code"));
}

#[test]
#[ignore]
fn test_pull_directory_real_api() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("wiki/");

    cmd()
        .args([
            "anthropics/claude-code",
            "--pages",
            "1",
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Done!"));

    // Check files were created
    let wiki_dir = output.join("anthropics-claude-code");
    assert!(wiki_dir.exists(), "Wiki directory should exist");

    let entries: Vec<_> = std::fs::read_dir(&wiki_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        !entries.is_empty(),
        "Should have at least one .md file in output"
    );
}
