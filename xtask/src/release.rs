#![allow(clippy::missing_errors_doc)]

use crate::git;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn prepare_release(repo_root: &Path, version: &str, date: &str) -> Result<()> {
    validate_semver(version)?;
    assert_clean_and_synced(repo_root)?;
    create_release_branch(repo_root, version)?;
    apply_release_edits(repo_root, version, date)?;
    run_release_gate(repo_root)?;
    commit_release(repo_root, version)?;
    println!("\nNext: cargo xtask cut-release {version}");
    Ok(())
}

pub fn cut_release(repo_root: &Path, version: &str) -> Result<()> {
    let branch = format!("release-{version}");
    git::run(repo_root, &["push", "-u", "origin", &branch])?;

    let origin_url = git::output(repo_root, &["remote", "get-url", "origin"])?;
    let fork_owner = parse_fork_owner(&origin_url)?;
    let body = format!(
        "## Summary\n- Bumps version to {version} and finalizes the CHANGELOG `[Unreleased]` section for this release.\n"
    );
    let status = Command::new("gh")
        .args([
            "pr",
            "create",
            "--repo",
            "mozilla/stmo-cli",
            "--base",
            "main",
            "--head",
            &format!("{fork_owner}:{branch}"),
            "--draft",
            "--title",
            &format!("Release {version}"),
            "--body",
            &body,
        ])
        .current_dir(repo_root)
        .status()
        .context("failed to run gh pr create")?;
    if !status.success() {
        anyhow::bail!("gh pr create failed");
    }

    println!("\nAfter the PR is merged:");
    println!("  git checkout main && git pull upstream main");
    println!("  git tag -s {version} -m \"{version}\"");
    println!("  git push upstream {version}");
    Ok(())
}

fn assert_clean_and_synced(repo_root: &Path) -> Result<()> {
    let porcelain_status = git::output(repo_root, &["status", "--porcelain"])?;
    if !porcelain_status.is_empty() {
        anyhow::bail!(
            "working tree is not clean; commit or stash changes before preparing a release"
        );
    }

    let branch = git::output(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if branch != "main" {
        anyhow::bail!("must be on `main` to prepare a release (currently on `{branch}`)");
    }

    git::run(repo_root, &["fetch", "upstream"])?;
    let head = git::output(repo_root, &["rev-parse", "HEAD"])?;
    let upstream_main = git::output(repo_root, &["rev-parse", "upstream/main"])?;
    if head != upstream_main {
        anyhow::bail!("`main` is not in sync with `upstream/main`; pull/rebase first");
    }
    Ok(())
}

fn run_release_gate(repo_root: &Path) -> Result<()> {
    run_cargo(repo_root, &["test", "--workspace"])?;
    run_cargo(
        repo_root,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-W",
            "clippy::pedantic",
            "-D",
            "warnings",
        ],
    )?;
    run_cargo(repo_root, &["fmt", "--check"])
}

fn run_cargo(repo_root: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("cargo")
        .args(args)
        .current_dir(repo_root)
        .status()
        .with_context(|| format!("failed to run cargo {args:?}"))?;
    if !status.success() {
        anyhow::bail!("cargo {args:?} failed");
    }
    Ok(())
}

pub fn validate_semver(version: &str) -> Result<()> {
    let parts: Vec<&str> = version.split('.').collect();
    let is_valid = parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));
    if !is_valid {
        anyhow::bail!("`{version}` is not a valid X.Y.Z semver version");
    }
    Ok(())
}

pub fn create_release_branch(repo_root: &Path, version: &str) -> Result<()> {
    git::run(
        repo_root,
        &["checkout", "-b", &format!("release-{version}")],
    )
}

pub fn apply_release_edits(repo_root: &Path, version: &str, date: &str) -> Result<()> {
    let manifest_path = repo_root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    std::fs::write(&manifest_path, bump_cargo_version(&manifest, version)?)
        .with_context(|| format!("writing {}", manifest_path.display()))?;

    let changelog_path = repo_root.join("CHANGELOG.md");
    let changelog = std::fs::read_to_string(&changelog_path)
        .with_context(|| format!("reading {}", changelog_path.display()))?;
    std::fs::write(
        &changelog_path,
        date_unreleased_heading(&changelog, version, date)?,
    )
    .with_context(|| format!("writing {}", changelog_path.display()))?;

    Ok(())
}

pub fn commit_release(repo_root: &Path, version: &str) -> Result<()> {
    git::run(repo_root, &["add", "-A"])?;
    git::run(repo_root, &["commit", "-m", &format!("Release {version}")])
}

pub fn parse_fork_owner(remote_url: &str) -> Result<String> {
    remote_url
        .rsplit(['/', ':'])
        .nth(1)
        .map(str::to_string)
        .with_context(|| format!("could not parse owner from remote URL `{remote_url}`"))
}

pub fn bump_cargo_version(manifest: &str, version: &str) -> Result<String> {
    let mut lines: Vec<&str> = manifest.lines().collect();
    let version_line = lines
        .iter()
        .position(|line| line.trim_start().starts_with("version = \""))
        .context("no `version = \"...\"` line in Cargo.toml")?;

    let new_line = format!("version = \"{version}\"");
    lines[version_line] = &new_line;
    Ok(format!("{}\n", lines.join("\n")))
}

pub fn date_unreleased_heading(changelog: &str, version: &str, date: &str) -> Result<String> {
    let new_heading = format!("## [{version}] - {date}");
    if !changelog.contains("## [Unreleased]") {
        anyhow::bail!("no `## [Unreleased]` section in CHANGELOG.md");
    }
    Ok(changelog.replacen("## [Unreleased]", &new_heading, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_cargo_version_rewrites_package_version() {
        let manifest = "\
[package]
name = \"stmo-cli\"
version = \"0.9.0\"
edition = \"2024\"
";
        let updated = bump_cargo_version(manifest, "0.10.0").unwrap();
        assert!(updated.contains("version = \"0.10.0\""));
        assert!(!updated.contains("0.9.0"));
    }

    #[test]
    fn bump_cargo_version_leaves_dependency_versions_untouched() {
        let manifest = "\
[package]
name = \"stmo-cli\"
version = \"0.9.0\"
edition = \"2024\"

[dependencies]
anyhow = { version = \"1\" }
";
        let updated = bump_cargo_version(manifest, "0.10.0").unwrap();
        assert!(updated.contains("version = \"0.10.0\""));
        assert!(updated.contains("anyhow = { version = \"1\" }"));
    }

    #[test]
    fn bump_cargo_version_errors_when_no_version_line() {
        let manifest = "[package]\nname = \"stmo-cli\"\n";
        assert!(bump_cargo_version(manifest, "0.10.0").is_err());
    }

    #[test]
    fn date_unreleased_heading_replaces_heading_and_preserves_body() {
        let changelog = "\
# Changelog

## [Unreleased]

### Features
- something new

## [0.9.0] - 2026-07-16

### Features
- old stuff
";
        let updated = date_unreleased_heading(changelog, "0.10.0", "2026-07-20").unwrap();
        assert!(updated.contains("## [0.10.0] - 2026-07-20"));
        assert!(!updated.contains("## [Unreleased]"));
        assert!(updated.contains("### Features\n- something new"));
        assert!(updated.contains("## [0.9.0] - 2026-07-16"));
    }

    #[test]
    fn date_unreleased_heading_errors_when_no_unreleased_section() {
        let changelog = "# Changelog\n\n## [0.9.0] - 2026-07-16\n\n### Features\n- old stuff\n";
        assert!(date_unreleased_heading(changelog, "0.10.0", "2026-07-20").is_err());
    }

    #[test]
    fn validate_semver_accepts_x_y_z() {
        assert!(validate_semver("0.10.0").is_ok());
        assert!(validate_semver("12.34.567").is_ok());
    }

    #[test]
    fn validate_semver_rejects_non_x_y_z() {
        assert!(validate_semver("0.10").is_err());
        assert!(validate_semver("v0.10.0").is_err());
        assert!(validate_semver("0.10.0-rc1").is_err());
        assert!(validate_semver("0.10.0.1").is_err());
        assert!(validate_semver("").is_err());
    }

    #[test]
    fn parse_fork_owner_from_ssh_url() {
        let owner = parse_fork_owner("git@github.com:JohanLorenzo/stmo-cli-fork.git").unwrap();
        assert_eq!(owner, "JohanLorenzo");
    }

    #[test]
    fn parse_fork_owner_from_https_url() {
        let owner = parse_fork_owner("https://github.com/JohanLorenzo/stmo-cli-fork.git").unwrap();
        assert_eq!(owner, "JohanLorenzo");
    }

    #[test]
    fn parse_fork_owner_from_https_url_without_git_suffix() {
        let owner = parse_fork_owner("https://github.com/JohanLorenzo/stmo-cli-fork").unwrap();
        assert_eq!(owner, "JohanLorenzo");
    }

    #[test]
    fn parse_fork_owner_rejects_unparseable_url() {
        assert!(parse_fork_owner("not-a-url").is_err());
    }
}
