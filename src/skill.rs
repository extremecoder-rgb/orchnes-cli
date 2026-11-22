//! Loading and parsing `skills/<name>/SKILL.md` files.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum description length allowed by the Agent Skills format.
pub const MAX_DESCRIPTION: usize = 1024;
/// Maximum name length allowed by the Agent Skills format.
pub const MAX_NAME: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0}: missing YAML frontmatter")]
    MissingFrontmatter(String),
    #[error("{0}: unterminated frontmatter")]
    UnterminatedFrontmatter(String),
    #[error("{skill}: invalid frontmatter YAML: {source}")]
    Yaml {
        skill: String,
        source: serde_norway::Error,
    },
    #[error("{skill}: name '{name}' != folder name")]
    NameMismatch { skill: String, name: String },
    #[error("{skill}: invalid name '{name}'")]
    InvalidName { skill: String, name: String },
    #[error("{skill}: description empty or > {MAX_DESCRIPTION} chars ({len})")]
    BadDescription { skill: String, len: usize },
}

/// Frontmatter fields of a SKILL.md.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Frontmatter {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub license: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub frontmatter: Frontmatter,
    /// The skill folder.
    pub dir: PathBuf,
    /// Full SKILL.md text, frontmatter included.
    pub raw: String,
    /// SKILL.md text after the frontmatter.
    pub body: String,
}

impl Skill {
    pub fn name(&self) -> &str {
        &self.frontmatter.name
    }

    pub fn description(&self) -> &str {
        &self.frontmatter.description
    }

    pub fn skill_md(&self) -> PathBuf {
        self.dir.join("SKILL.md")
    }
}

/// Splits SKILL.md text into (frontmatter YAML, body). `label` names the skill in errors.
pub fn split_frontmatter<'a>(text: &'a str, label: &str) -> Result<(&'a str, &'a str), SkillError> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| SkillError::MissingFrontmatter(label.into()))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| SkillError::UnterminatedFrontmatter(label.into()))?;
    let after = &rest[end + 4..];
    // Drop the rest of the closing `---` line and leading blank lines.
    let body = after.split_once('\n').map_or("", |(_, b)| b);
    Ok((&rest[..end], body.trim_start_matches('\n')))
}

pub fn parse_frontmatter(yaml: &str, label: &str) -> Result<Frontmatter, SkillError> {
    serde_norway::from_str(yaml).map_err(|source| SkillError::Yaml {
        skill: label.into(),
        source,
    })
}

/// Lowercase letters, digits and single hyphens, not starting or ending with a hyphen.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Loads one skill folder and checks name and description.
pub fn load_skill(dir: &Path) -> Result<Skill, SkillError> {
    let folder = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let path = dir.join("SKILL.md");
    let raw = fs::read_to_string(&path).map_err(|source| SkillError::Io { path, source })?;
    let (yaml, body) = split_frontmatter(&raw, &folder)?;
    let frontmatter = parse_frontmatter(yaml, &folder)?;
    let body = body.to_string();
    check_frontmatter(&frontmatter, &folder)?;
    Ok(Skill {
        frontmatter,
        dir: dir.to_path_buf(),
        raw,
        body,
    })
}

/// Name and description rules shared by loading and `validate`. Returns every violation.
pub fn frontmatter_errors(fm: &Frontmatter, folder: &str) -> Vec<SkillError> {
    let mut errors = Vec::new();
    if !is_valid_name(&fm.name) {
        errors.push(SkillError::InvalidName {
            skill: folder.into(),
            name: fm.name.clone(),
        });
    }
    if fm.name != folder {
        errors.push(SkillError::NameMismatch {
            skill: folder.into(),
            name: fm.name.clone(),
        });
    }
    let len = fm.description.chars().count();
    if len == 0 || len > MAX_DESCRIPTION {
        errors.push(SkillError::BadDescription {
            skill: folder.into(),
            len,
        });
    }
    errors
}

fn check_frontmatter(fm: &Frontmatter, folder: &str) -> Result<(), SkillError> {
    match frontmatter_errors(fm, folder).into_iter().next() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Subdirectories of `skills_dir`, sorted by name.
pub fn skill_dirs(skills_dir: &Path) -> Result<Vec<PathBuf>, SkillError> {
    let io = |source| SkillError::Io {
        path: skills_dir.to_path_buf(),
        source,
    };
    let mut dirs = Vec::new();
    for entry in fs::read_dir(skills_dir).map_err(io)? {
        let path = entry.map_err(io)?.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

/// Loads every skill under `skills_dir`, sorted by folder name. Fails on the first invalid skill.
pub fn load_all(skills_dir: &Path) -> Result<Vec<Skill>, SkillError> {
    skill_dirs(skills_dir)?
        .iter()
        .map(|d| load_skill(d))
        .collect()
}

/// Text up to and including the first sentence end (`. ` or end of text), or the
/// part before an em-dash list if that comes first.
pub fn first_sentence(text: &str) -> &str {
    let text = text.trim();
    let end = [". ", " — "]
        .iter()
        .filter_map(|sep| text.find(sep).map(|i| if *sep == ". " { i + 1 } else { i }))
        .min()
        .unwrap_or(text.len());
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "---\nname: demo\ndescription: Does a thing. Use it often.\nlicense: MIT\n---\n\n# Demo\nBody.\n";

    #[test]
    fn splits_and_parses_frontmatter() {
        let (yaml, body) = split_frontmatter(GOOD, "demo").unwrap();
        let fm = parse_frontmatter(yaml, "demo").unwrap();
        assert_eq!(fm.name, "demo");
        assert_eq!(fm.description, "Does a thing. Use it often.");
        assert_eq!(fm.license.as_deref(), Some("MIT"));
        assert_eq!(body, "# Demo\nBody.\n");
    }

    #[test]
    fn rejects_missing_or_unterminated_frontmatter() {
        assert!(matches!(
            split_frontmatter("# no fm", "x"),
            Err(SkillError::MissingFrontmatter(_))
        ));
        assert!(matches!(
            split_frontmatter("---\nname: x\n", "x"),
            Err(SkillError::UnterminatedFrontmatter(_))
        ));
    }

    #[test]
    fn name_rules() {
        assert!(is_valid_name("agent-behavior"));
        assert!(is_valid_name("a1"));
        for bad in ["", "-a", "a-", "a--b", "Ab", "a_b", &"a".repeat(65)] {
            assert!(!is_valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn frontmatter_errors_reports_all_violations() {
        let fm = Frontmatter {
            name: "Other".into(),
            description: "x".repeat(MAX_DESCRIPTION + 1),
            license: None,
        };
        let errs: Vec<String> = frontmatter_errors(&fm, "demo")
            .iter()
            .map(|e| e.to_string())
            .collect();
        assert_eq!(errs.len(), 3, "{errs:?}");
        assert!(errs[1].contains("!= folder name"));
        assert!(errs[2].contains("(1025)"));
    }

    #[test]
    fn first_sentence_cases() {
        assert_eq!(first_sentence("One. Two."), "One.");
        assert_eq!(first_sentence("No end"), "No end");
        assert_eq!(first_sentence("Lead — detail. More."), "Lead");
        assert_eq!(first_sentence("v1.2 is out. Yes."), "v1.2 is out.");
    }

    #[test]
    fn load_skill_checks_folder_name() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("wrong");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), GOOD).unwrap();
        let err = load_skill(&dir).unwrap_err();
        assert!(matches!(err, SkillError::NameMismatch { .. }), "{err}");
    }
}
