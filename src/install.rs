//! Copy skill folders into an agent's native skills directory.

use crate::export::{ExportError, files_under};
use crate::skill::Skill;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Target {
    /// .agents/skills (read by Codex, Cursor, Copilot, Gemini CLI, Windsurf)
    Agents,
    Claude,
    Cline,
    /// same as agents
    Codex,
    Cursor,
    Copilot,
    Gemini,
    Windsurf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Scope {
    Project,
    User,
}

/// Skills directory relative to the project root (project scope) or home (user scope).
pub fn skills_subdir(target: Target, scope: Scope) -> &'static str {
    use {Scope::*, Target::*};
    match (target, scope) {
        (Agents | Codex, _) | (Windsurf, User) => ".agents/skills",
        (Claude, _) => ".claude/skills",
        (Cline, _) => ".cline/skills",
        (Cursor, _) => ".cursor/skills",
        (Copilot, Project) => ".github/skills",
        (Copilot, User) => ".copilot/skills",
        (Gemini, _) => ".gemini/skills",
        (Windsurf, Project) => ".windsurf/skills",
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("{0} already exists (use --force to replace it)")]
    Exists(PathBuf),
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Copies each skill folder to `<dest>/<name>`. Existing folders are replaced only with `force`.
/// Returns the installed folders.
pub fn install(skills: &[Skill], dest: &Path, force: bool) -> Result<Vec<PathBuf>, InstallError> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| InstallError::Io { path, source }
    };
    if !force
        && let Some(existing) = skills
            .iter()
            .map(|s| dest.join(s.name()))
            .find(|p| p.exists())
    {
        return Err(InstallError::Exists(existing));
    }
    let mut installed = Vec::new();
    for sk in skills {
        let target = dest.join(sk.name());
        if target.exists() {
            fs::remove_dir_all(&target).map_err(io(&target))?;
        }
        for file in files_under(&sk.dir)? {
            let to = target.join(file.strip_prefix(&sk.dir).unwrap_or(&file));
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(io(parent))?;
            }
            fs::copy(&file, &to).map_err(io(&file))?;
        }
        installed.push(target);
    }
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::load_all;

    #[test]
    fn paths_per_target() {
        assert_eq!(
            skills_subdir(Target::Codex, Scope::Project),
            ".agents/skills"
        );
        assert_eq!(
            skills_subdir(Target::Copilot, Scope::Project),
            ".github/skills"
        );
        assert_eq!(
            skills_subdir(Target::Copilot, Scope::User),
            ".copilot/skills"
        );
        assert_eq!(
            skills_subdir(Target::Windsurf, Scope::Project),
            ".windsurf/skills"
        );
        assert_eq!(
            skills_subdir(Target::Windsurf, Scope::User),
            ".agents/skills"
        );
        assert_eq!(skills_subdir(Target::Claude, Scope::User), ".claude/skills");
    }

    #[test]
    fn copies_folders_and_refuses_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("skills/demo");
        fs::create_dir_all(src.join("references")).unwrap();
        fs::write(
            src.join("SKILL.md"),
            "---\nname: demo\ndescription: D.\n---\nx\n",
        )
        .unwrap();
        fs::write(src.join("references/r.md"), "ref").unwrap();
        let skills = load_all(&tmp.path().join("skills")).unwrap();
        let dest = tmp.path().join("out/.claude/skills");

        install(&skills, &dest, false).unwrap();
        assert_eq!(
            fs::read_to_string(dest.join("demo/references/r.md")).unwrap(),
            "ref"
        );

        fs::write(dest.join("demo/stale.md"), "old").unwrap();
        assert!(matches!(
            install(&skills, &dest, false),
            Err(InstallError::Exists(_))
        ));
        install(&skills, &dest, true).unwrap();
        assert!(!dest.join("demo/stale.md").exists());
    }
}
