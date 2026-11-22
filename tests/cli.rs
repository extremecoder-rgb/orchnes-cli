//! Runs the `orchnes` binary against a temp fixture and, when present, the real
//! orchnes-360 repo (read-only). Set ORCHNES_SKILLS_DIR to point at its `skills/` folder;
//! default is `../orchnes-360/skills` next to this crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn orchnes(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_orchnes"))
        .args(args)
        .output()
        .expect("run orchnes")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// The real orchnes-360 `skills/` folder, or None (test prints why it is skipped).
fn real_skills() -> Option<String> {
    let dir = std::env::var("ORCHNES_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../orchnes-360/skills"));
    if dir.is_dir() {
        Some(dir.to_string_lossy().into_owned())
    } else {
        eprintln!("SKIPPED: orchnes-360 not found at {}", dir.display());
        None
    }
}

fn write_skill(skills: &Path, folder: &str, text: &str) {
    fs::create_dir_all(skills.join(folder)).unwrap();
    fs::write(skills.join(folder).join("SKILL.md"), text).unwrap();
}

/// One valid skill and one broken skill (wrong name, missing reference).
fn fixture() -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().unwrap();
    let skills = tmp.path().join("skills");
    write_skill(
        &skills,
        "good",
        "---\nname: good\ndescription: Works well. Really.\n---\n# Good\n",
    );
    write_skill(
        &skills,
        "broken",
        "---\nname: Broken_Name\ndescription: Oops.\n---\nSee `references/missing.md`.\n",
    );
    let dir = skills.to_string_lossy().into_owned();
    (tmp, dir)
}

#[test]
fn validate_reports_broken_fixture() {
    let (_tmp, dir) = fixture();
    let o = orchnes(&["validate", "--skills-dir", &dir]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        stdout(&o),
        "ERROR broken: invalid name 'Broken_Name'\n\
         ERROR broken: name 'Broken_Name' != folder name\n\
         ERROR broken: referenced file missing: references/missing.md\n\
         2 skills checked, 3 errors\n"
    );
}

#[test]
fn list_fails_on_broken_fixture_and_works_without_it() {
    let (tmp, dir) = fixture();
    let o = orchnes(&["list", "--skills-dir", &dir]);
    assert!(!o.status.success());
    assert!(
        stderr(&o).contains("broken: invalid name 'Broken_Name'"),
        "{}",
        stderr(&o)
    );

    fs::remove_dir_all(tmp.path().join("skills/broken")).unwrap();
    let o = orchnes(&["list", "--skills-dir", &dir]);
    assert!(o.status.success());
    assert_eq!(
        stdout(&o).split_whitespace().collect::<Vec<_>>(),
        ["good", "Works", "well."]
    );
}

#[test]
fn show_rejects_path_like_names() {
    let (_tmp, dir) = fixture();
    let o = orchnes(&["show", "../good", "--skills-dir", &dir]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("invalid skill name"));
}

#[test]
fn route_tiers() {
    let run = |c: &str, r: &str, x: &str, k: &str, extra: &[&str]| {
        let mut args = vec![
            "route",
            "--complexity",
            c,
            "--risk",
            r,
            "--context",
            x,
            "--capability",
            k,
        ];
        args.extend_from_slice(extra);
        stdout(&orchnes(&args))
    };
    assert_eq!(
        run("1", "2", "1", "2", &[]),
        "tier: T3\nreason: all factors ≤ 2, sum 6 (≤ 6 → T3)\n"
    );
    assert!(run("2", "2", "2", "1", &[]).starts_with("tier: T2\n"));
    assert_eq!(
        run("1", "3", "1", "1", &["--sensitive"]),
        "tier: T1\nreason: risk = 3 (any factor 3 → T1)\nproviders: local/compliant providers only\n"
    );
    let o = orchnes(&[
        "route",
        "--complexity",
        "4",
        "--risk",
        "1",
        "--context",
        "1",
        "--capability",
        "1",
    ]);
    assert!(!o.status.success());
}

#[test]
fn install_into_user_scope_with_temp_home() {
    let (tmp, dir) = fixture();
    fs::remove_dir_all(tmp.path().join("skills/broken")).unwrap();
    let home = tmp.path().join("home");
    let o = Command::new(env!("CARGO_BIN_EXE_orchnes"))
        .args([
            "install",
            "--target",
            "copilot",
            "--scope",
            "user",
            "--skills-dir",
            &dir,
        ])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(home.join(".copilot/skills/good/SKILL.md").is_file());

    let dest = tmp.path().join("proj").to_string_lossy().into_owned();
    let args = [
        "install",
        "--target",
        "codex",
        "--dest",
        &dest,
        "--skills-dir",
        &dir,
        "good",
    ];
    assert!(orchnes(&args).status.success());
    assert!(
        tmp.path()
            .join("proj/.agents/skills/good/SKILL.md")
            .is_file()
    );
    let again = orchnes(&args);
    assert!(!again.status.success());
    assert!(stderr(&again).contains("--force"));
}

#[test]
fn real_repo_validate_list_show() {
    let Some(dir) = real_skills() else { return };
    let o = orchnes(&["validate", "--skills-dir", &dir]);
    assert!(o.status.success(), "{}", stdout(&o));
    assert!(
        stdout(&o).ends_with("26 skills checked, 0 errors\n"),
        "{}",
        stdout(&o)
    );

    let list = stdout(&orchnes(&["list", "--skills-dir", &dir]));
    assert_eq!(list.lines().count(), 26);
    assert!(list.lines().any(|l| l.starts_with("agent-orchestra ")));

    let shown = orchnes(&["show", "agent-behavior", "--skills-dir", &dir]);
    let file = fs::read_to_string(Path::new(&dir).join("agent-behavior/SKILL.md")).unwrap();
    assert_eq!(stdout(&shown), file);
}

#[test]
fn real_repo_export_every_format() {
    let Some(dir) = real_skills() else { return };
    let out = tempfile::tempdir().unwrap();
    let expected = [
        ("agents-md", 1, "AGENTS.md"),
        ("bundle", 1, "SKILLS.md"),
        ("gemini", 1, "GEMINI.md"),
        ("aider", 1, "CONVENTIONS.md"),
        ("cursor", 26, ".cursor/rules/agent-behavior.mdc"),
        ("copilot", 1, ".github/copilot-instructions.md"),
        ("windsurf", 26, ".windsurf/rules/memory.md"),
        ("continue", 26, ".continue/rules/research.md"),
        ("claude-zip", 26, "zips/agent-orchestra.zip"),
    ];
    for (format, count, sample) in expected {
        let target = if format == "claude-zip" {
            out.path().join("zips")
        } else {
            out.path().to_path_buf()
        };
        let target = target.to_string_lossy().into_owned();
        let o = orchnes(&[
            "export",
            "--format",
            format,
            "--out",
            &target,
            "--skills-dir",
            &dir,
        ]);
        assert!(o.status.success(), "{format}: {}", stderr(&o));
        assert!(
            stdout(&o).starts_with(&format!("wrote {count} files")),
            "{format}: {}",
            stdout(&o)
        );
        assert!(out.path().join(sample).is_file(), "{format}: {sample}");
    }
    let bundle = fs::read_to_string(out.path().join("SKILLS.md")).unwrap();
    assert_eq!(bundle.matches("\n---\n\n## ").count(), 26);
    let cursor = fs::read_to_string(out.path().join(".cursor/rules/agent-behavior.mdc")).unwrap();
    assert!(cursor.contains("\nalwaysApply: true\n"));
}
