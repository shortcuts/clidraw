#![cfg(unix)]

use clidraw::conventions::CONVENTIONS;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::{Command, Output};

fn fake_claude(tmp: &Path) {
    let bin = tmp.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let script = bin.join("claude");
    fs::write(
        &script,
        format!("#!/bin/sh\necho \"$*\" >> {}/claude.log\n", tmp.display()),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
}

fn setup(tmp: &Path, path: &Path, config_dir: Option<&Path>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_clidraw"));
    cmd.arg("setup")
        .args(args)
        .env("HOME", tmp)
        .env("PATH", path)
        .env_remove("CLAUDE_CONFIG_DIR");
    if let Some(dir) = config_dir {
        cmd.env("CLAUDE_CONFIG_DIR", dir);
    }
    cmd.output().unwrap()
}

fn with_fake(tmp: &Path, config_dir: Option<&Path>) -> Output {
    fake_claude(tmp);
    setup(tmp, &tmp.join("bin"), config_dir, &["--claude"])
}

fn block() -> String {
    format!("<!-- clidraw:begin -->\n## clidraw\n\n{CONVENTIONS}<!-- clidraw:end -->\n")
}

#[test]
fn registers_the_server_and_writes_the_block_under_home() {
    let t = tempfile::tempdir().unwrap();
    let out = with_fake(t.path(), None);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        fs::read_to_string(t.path().join(".claude/CLAUDE.md")).unwrap(),
        block()
    );
    assert_eq!(
        fs::read_to_string(t.path().join("claude.log")).unwrap(),
        "mcp remove --scope user clidraw\nmcp add --scope user clidraw -- clidraw mcp\n"
    );
}

#[test]
fn claude_config_dir_moves_the_block() {
    let t = tempfile::tempdir().unwrap();
    let alt = t.path().join("alt");
    assert!(with_fake(t.path(), Some(&alt)).status.success());
    assert_eq!(fs::read_to_string(alt.join("CLAUDE.md")).unwrap(), block());
    assert!(!t.path().join(".claude").exists());
}

#[test]
fn rerun_keeps_text_outside_the_markers() {
    let t = tempfile::tempdir().unwrap();
    let alt = t.path().join("alt");
    fs::create_dir_all(&alt).unwrap();
    let file = alt.join("CLAUDE.md");
    fs::write(
        &file,
        "above\n<!-- clidraw:begin -->\nstale\n<!-- clidraw:end -->\nbelow\n",
    )
    .unwrap();
    assert!(with_fake(t.path(), Some(&alt)).status.success());
    let first = fs::read_to_string(&file).unwrap();
    assert_eq!(first, format!("above\n{}below\n", block()));
    assert!(with_fake(t.path(), Some(&alt)).status.success());
    assert_eq!(fs::read_to_string(&file).unwrap(), first);
}

#[test]
fn symlinked_claude_md_stays_a_symlink() {
    let t = tempfile::tempdir().unwrap();
    let alt = t.path().join("alt");
    fs::create_dir_all(&alt).unwrap();
    let target = t.path().join("dotfiles.md");
    fs::write(&target, "mine\n").unwrap();
    symlink(&target, alt.join("CLAUDE.md")).unwrap();
    assert!(with_fake(t.path(), Some(&alt)).status.success());
    assert!(fs::symlink_metadata(alt.join("CLAUDE.md"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        format!("mine\n\n{}", block())
    );
}

#[test]
fn lone_marker_fails_and_changes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let alt = t.path().join("alt");
    fs::create_dir_all(&alt).unwrap();
    let file = alt.join("CLAUDE.md");
    fs::write(&file, "<!-- clidraw:begin -->\nhalf\n").unwrap();
    let out = with_fake(t.path(), Some(&alt));
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("CLAUDE.md"));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "<!-- clidraw:begin -->\nhalf\n"
    );
    assert!(!t.path().join("claude.log").exists());
}

#[test]
fn missing_claude_binary_fails_without_writing() {
    let t = tempfile::tempdir().unwrap();
    let empty = t.path().join("empty");
    fs::create_dir_all(&empty).unwrap();
    let out = setup(t.path(), &empty, None, &["--claude"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("claude"));
    assert!(!t.path().join(".claude/CLAUDE.md").exists());
}

#[test]
fn no_flag_prints_usage() {
    let t = tempfile::tempdir().unwrap();
    let out = setup(t.path(), &t.path().join("bin"), None, &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--claude"));
}
