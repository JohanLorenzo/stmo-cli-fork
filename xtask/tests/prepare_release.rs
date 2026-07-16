use std::fs;
use tempfile::TempDir;
use xtask::git;
use xtask::release::{apply_release_edits, commit_release, create_release_branch};

fn setup_test_repo(dir: &std::path::Path) {
    git::clean_git_cmd()
        .arg("init")
        .arg("--initial-branch=main")
        .current_dir(dir)
        .status()
        .unwrap();
    git::clean_git_cmd()
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .status()
        .unwrap();
    git::clean_git_cmd()
        .args(["config", "user.email", "test@test"])
        .current_dir(dir)
        .status()
        .unwrap();
}

const FIXTURE_CARGO_TOML: &str = "\
[package]
name = \"stmo-cli\"
version = \"0.9.0\"
edition = \"2024\"
";

const FIXTURE_CHANGELOG: &str = "\
# Changelog

## [Unreleased]

### Features
- something new

## [0.9.0] - 2026-07-16

### Features
- old stuff
";

#[test]
fn prepare_release_apply_layer_creates_branch_edits_files_and_commits() {
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    setup_test_repo(repo_root);

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_CARGO_TOML).unwrap();
    fs::write(repo_root.join("CHANGELOG.md"), FIXTURE_CHANGELOG).unwrap();
    git::clean_git_cmd()
        .args(["add", "-A"])
        .current_dir(repo_root)
        .status()
        .unwrap();
    git::clean_git_cmd()
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_root)
        .status()
        .unwrap();

    create_release_branch(repo_root, "0.10.0").unwrap();
    apply_release_edits(repo_root, "0.10.0", "2026-07-20").unwrap();
    commit_release(repo_root, "0.10.0").unwrap();

    let branch = git::output(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
    assert_eq!(branch, "release-0.10.0");

    let cargo_toml = fs::read_to_string(repo_root.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains("version = \"0.10.0\""));

    let changelog = fs::read_to_string(repo_root.join("CHANGELOG.md")).unwrap();
    assert!(changelog.contains("## [0.10.0] - 2026-07-20"));
    assert!(!changelog.contains("## [Unreleased]"));

    let last_commit_message = git::output(repo_root, &["log", "-1", "--format=%s"]).unwrap();
    assert_eq!(last_commit_message, "Release 0.10.0");
}

#[test]
fn create_release_branch_fails_clearly_when_branch_already_exists() {
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    setup_test_repo(repo_root);

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_CARGO_TOML).unwrap();
    git::clean_git_cmd()
        .args(["add", "-A"])
        .current_dir(repo_root)
        .status()
        .unwrap();
    git::clean_git_cmd()
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_root)
        .status()
        .unwrap();
    git::clean_git_cmd()
        .args(["branch", "release-0.10.0"])
        .current_dir(repo_root)
        .status()
        .unwrap();

    assert!(create_release_branch(repo_root, "0.10.0").is_err());
}
