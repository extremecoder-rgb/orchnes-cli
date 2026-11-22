//! The checks of agent-skills' `scripts/validate.py`, reimplemented in Rust.
//!
//! Repo-level checks (evals file present, skill listed in README.md) run only when
//! the skills folder's parent contains an `evals/` folder or a `README.md`.

use crate::skill::{self, SkillError};
use std::fs;
use std::path::Path;

/// Keep SKILL.md lean; detail belongs in references/.
pub const MAX_LINES: usize = 500;

pub struct Report {
    pub checked: usize,
    pub errors: Vec<String>,
}

pub fn validate(skills_dir: &Path) -> Result<Report, SkillError> {
    let dirs = skill::skill_dirs(skills_dir)?;
    let root = skills_dir.parent().unwrap_or(Path::new("."));
    let readme = fs::read_to_string(root.join("README.md")).ok();
    let evals = root.join("evals");
    let evals = evals.is_dir().then_some(evals);
    let errors = dirs
        .iter()
        .flat_map(|d| check_skill(d, readme.as_deref(), evals.as_deref()))
        .collect();
    Ok(Report {
        checked: dirs.len(),
        errors,
    })
}

fn check_skill(dir: &Path, readme: Option<&str>, evals: Option<&Path>) -> Vec<String> {
    let folder = dir.file_name().unwrap_or_default().to_string_lossy();
    let Ok(text) = fs::read_to_string(dir.join("SKILL.md")) else {
        return vec![format!("{folder}: SKILL.md missing")];
    };
    let fm = skill::split_frontmatter(&text, &folder)
        .and_then(|(yaml, _)| skill::parse_frontmatter(yaml, &folder));
    let fm = match fm {
        Ok(fm) => fm,
        Err(e) => return vec![e.to_string()],
    };
    let mut errors: Vec<String> = skill::frontmatter_errors(&fm, &folder)
        .iter()
        .map(|e| e.to_string())
        .collect();
    if text.lines().count() > MAX_LINES {
        errors.push(format!("{folder}: SKILL.md over {MAX_LINES} lines"));
    }
    if let Some(evals) = evals
        && !evals.join(format!("{folder}.md")).exists()
    {
        errors.push(format!("{folder}: missing evals/{folder}.md"));
    }
    if let Some(readme) = readme
        && !readme.contains(&format!("skills/{folder}/SKILL.md"))
    {
        errors.push(format!("{folder}: not listed in README.md"));
    }
    for r in referenced_files(&text) {
        if !dir.join(r).exists() {
            errors.push(format!("{folder}: referenced file missing: {r}"));
        }
    }
    errors
}

/// Paths written as `` `references/...` `` (same as the regex `` `(references/[^`]+)` ``).
pub fn referenced_files(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("`references/") {
        let after = &rest[i + 1..];
        let Some(j) = after.find('`') else { break };
        found.push(&after[..j]);
        rest = &after[j + 1..];
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_backticked_references_only() {
        let text = "see `references/a.md`, not references/b.md, and `x` `references/c/d.md`";
        assert_eq!(
            referenced_files(text),
            vec!["references/a.md", "references/c/d.md"]
        );
        assert!(referenced_files("`references/unclosed").is_empty());
    }

    fn write_skill(root: &Path, folder: &str, text: &str) {
        let dir = root.join("skills").join(folder);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), text).unwrap();
    }

    #[test]
    fn reports_each_problem() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_skill(
            root,
            "ok",
            "---\nname: ok\ndescription: Fine.\n---\nSee `references/x.md`.\n",
        );
        fs::create_dir(root.join("skills/ok/references")).unwrap();
        fs::write(root.join("skills/ok/references/x.md"), "x").unwrap();
        let long = format!(
            "---\nname: bad\ndescription: Fine.\n---\n{}`references/gone.md`\n",
            "line\n".repeat(MAX_LINES)
        );
        write_skill(root, "bad", &long);
        write_skill(root, "nofm", "# nothing\n");
        fs::create_dir_all(root.join("skills/empty")).unwrap();

        let report = validate(&root.join("skills")).unwrap();
        assert_eq!(report.checked, 4);
        assert_eq!(
            report.errors,
            vec![
                "bad: SKILL.md over 500 lines",
                "bad: referenced file missing: references/gone.md",
                "empty: SKILL.md missing",
                "nofm: missing YAML frontmatter",
            ]
        );
    }

    #[test]
    fn repo_checks_apply_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_skill(root, "ok", "---\nname: ok\ndescription: Fine.\n---\n");
        fs::create_dir(root.join("evals")).unwrap();
        fs::write(root.join("README.md"), "# Skills\n").unwrap();
        let report = validate(&root.join("skills")).unwrap();
        assert_eq!(
            report.errors,
            vec!["ok: missing evals/ok.md", "ok: not listed in README.md"]
        );
    }
}
