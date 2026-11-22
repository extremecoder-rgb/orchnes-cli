//! Export skills to other agents' instruction formats.
//!
//! Each format is one small function returning the files to write (path relative to
//! the output dir + bytes), so a format's paths or frontmatter can change in one place.

use crate::skill::Skill;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

/// The skill applied to every request where a format supports "always on".
pub const ALWAYS_ON: &str = "agent-behavior";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// AGENTS.md: intro + index of skills with paths to SKILL.md
    AgentsMd,
    /// SKILLS.md: every SKILL.md body with a table of contents
    Bundle,
    /// .cursor/rules/<name>.mdc per skill
    Cursor,
    /// .github/copilot-instructions.md: index of skills + agent-behavior (install skills natively with `orchnes install --target copilot`)
    Copilot,
    /// GEMINI.md (same content as agents-md)
    Gemini,
    /// <name>.zip per skill folder, for Claude.ai upload
    ClaudeZip,
    /// .windsurf/rules/<name>.md per skill
    Windsurf,
    /// .continue/rules/<name>.md per skill
    Continue,
    /// CONVENTIONS.md (same content as bundle)
    Aider,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> ExportError + '_ {
    move |source| ExportError::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub struct OutFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

fn text(path: impl Into<PathBuf>, s: String) -> OutFile {
    OutFile {
        path: path.into(),
        bytes: s.into_bytes(),
    }
}

/// Renders `format` and writes it under `out`. Returns the written paths.
pub fn export(skills: &[Skill], format: Format, out: &Path) -> Result<Vec<PathBuf>, ExportError> {
    fs::create_dir_all(out).map_err(io_err(out))?;
    let files = render(skills, format)?;
    let mut written = Vec::with_capacity(files.len());
    for f in files {
        let path = out.join(&f.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        fs::write(&path, &f.bytes).map_err(io_err(&path))?;
        written.push(path);
    }
    Ok(written)
}

/// Native install location used in generated index files (`orchnes install --target agents`).
const SKILLS_DIR: &str = ".agents/skills";

/// Files for `format`, with paths relative to the output dir.
pub fn render(skills: &[Skill], format: Format) -> Result<Vec<OutFile>, ExportError> {
    Ok(match format {
        Format::AgentsMd => vec![agents_md(skills, "AGENTS.md")],
        Format::Gemini => vec![agents_md(skills, "GEMINI.md")],
        Format::Bundle => vec![bundle(skills, "SKILLS.md")],
        Format::Aider => vec![bundle(skills, "CONVENTIONS.md")],
        Format::Cursor => skills.iter().map(cursor_rule).collect(),
        Format::Copilot => copilot(skills),
        Format::Windsurf => skills.iter().map(windsurf_rule).collect(),
        Format::Continue => skills.iter().map(continue_rule).collect(),
        Format::ClaudeZip => skills.iter().map(claude_zip).collect::<Result<_, _>>()?,
    })
}

/// `- **name** — description (link)` per skill.
fn index(skills: &[Skill], link: impl Fn(&Skill) -> String) -> String {
    skills
        .iter()
        .map(|s| format!("- **{}** — {} (`{}`)\n", s.name(), s.description(), link(s)))
        .collect()
}

fn agents_md(skills: &[Skill], file: &str) -> OutFile {
    let mut s = format!(
        "# Agent skills\n\n\
         This project uses Agent Skills, installed in `{SKILLS_DIR}/` (run `orchnes install --target agents`).\n\
         Each skill is a folder with a SKILL.md and optional references/.\n\
         When a task matches a skill's description, read that skill's SKILL.md before starting and follow it;\n\
         read files under its references/ only when SKILL.md points to them.\n\n## Skills\n\n",
    );
    s += &index(skills, |sk| format!("{SKILLS_DIR}/{}/SKILL.md", sk.name()));
    text(file, s)
}

fn bundle(skills: &[Skill], file: &str) -> OutFile {
    let mut s = String::from(
        "# Skills\n\nAll skills in one file. Apply a skill when the task matches its description.\n\n## Contents\n\n",
    );
    for sk in skills {
        s += &format!("- [{0}](#{0})\n", sk.name());
    }
    for sk in skills {
        s += &format!(
            "\n---\n\n## {}\n\n> {}\n\n{}",
            sk.name(),
            sk.description(),
            sk.body
        );
        if !s.ends_with('\n') {
            s.push('\n');
        }
    }
    text(file, s)
}

/// Rule files carry only the SKILL.md body; point to the installed folder for references/.
fn rule_body(sk: &Skill) -> String {
    if sk.dir.join("references").is_dir() {
        format!(
            "> Reference files mentioned below are in `{SKILLS_DIR}/{}/references/` (install with `orchnes install --target agents`).\n\n{}",
            sk.name(),
            sk.body
        )
    } else {
        sk.body.clone()
    }
}

fn cursor_rule(sk: &Skill) -> OutFile {
    let always = sk.name() == ALWAYS_ON;
    text(
        format!(".cursor/rules/{}.mdc", sk.name()),
        format!(
            "---\ndescription: {}\nglobs:\nalwaysApply: {always}\n---\n\n{}",
            sk.description(),
            rule_body(sk)
        ),
    )
}

fn copilot(skills: &[Skill]) -> Vec<OutFile> {
    // Copilot loads Agent Skills natively from .github/skills/; per-skill instruction files with
    // applyTo "**" would inject every skill into every request, so only the index is exported.
    let mut main = String::from(
        "# Copilot instructions\n\n\
         Agent Skills are installed in `.github/skills/` (run `orchnes install --target copilot`).\n\
         When a task matches a skill's description, follow that skill.\n\n## Skills\n\n",
    );
    main += &index(skills, |sk| {
        format!(".github/skills/{}/SKILL.md", sk.name())
    });
    if let Some(core) = skills.iter().find(|s| s.name() == ALWAYS_ON) {
        main += &format!("\n## {ALWAYS_ON}\n\n{}", core.body);
    }
    vec![text(".github/copilot-instructions.md", main)]
}

fn windsurf_rule(sk: &Skill) -> OutFile {
    let trigger = if sk.name() == ALWAYS_ON {
        "always_on"
    } else {
        "model_decision"
    };
    text(
        format!(".windsurf/rules/{}.md", sk.name()),
        format!(
            "---\ntrigger: {trigger}\ndescription: {}\n---\n\n{}",
            sk.description(),
            rule_body(sk)
        ),
    )
}

fn continue_rule(sk: &Skill) -> OutFile {
    text(
        format!(".continue/rules/{}.md", sk.name()),
        format!(
            "---\nname: {}\ndescription: {}\nalwaysApply: {}\n---\n\n{}",
            sk.name(),
            sk.description(),
            sk.name() == ALWAYS_ON,
            rule_body(sk)
        ),
    )
}

/// `<name>.zip` with the skill folder at the archive root (`<name>/SKILL.md`, ...).
fn claude_zip(sk: &Skill) -> Result<OutFile, ExportError> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for file in files_under(&sk.dir)? {
        let rel = file.strip_prefix(&sk.dir).unwrap_or(&file);
        let name: Vec<String> = std::iter::once(sk.name().to_string())
            .chain(
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().into()),
            )
            .collect();
        zip.start_file(name.join("/"), opts)?;
        zip.write_all(&fs::read(&file).map_err(io_err(&file))?)
            .map_err(io_err(&file))?;
    }
    Ok(OutFile {
        path: format!("{}.zip", sk.name()).into(),
        bytes: zip.finish()?.into_inner(),
    })
}

/// All files under `dir`, recursively, sorted.
pub fn files_under(dir: &Path) -> Result<Vec<PathBuf>, ExportError> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).map_err(io_err(&d))? {
            let path = entry.map_err(io_err(&d))?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::load_all;

    fn fixture() -> (tempfile::TempDir, Vec<Skill>) {
        let tmp = tempfile::tempdir().unwrap();
        for (name, desc) in [("agent-behavior", "Core rules."), ("demo", "Demo skill.")] {
            let dir = tmp.path().join("skills").join(name);
            fs::create_dir_all(dir.join("references")).unwrap();
            fs::write(
                dir.join("SKILL.md"),
                format!(
                    "---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\nBody of {name}.\n"
                ),
            )
            .unwrap();
            fs::write(dir.join("references/r.md"), "ref").unwrap();
        }
        let skills = load_all(&tmp.path().join("skills")).unwrap();
        (tmp, skills)
    }

    fn render_one(skills: &[Skill], format: Format, _out: &Path, path: &str) -> String {
        let files = render(skills, format).unwrap();
        let f = files
            .iter()
            .find(|f| f.path == Path::new(path))
            .unwrap_or_else(|| panic!("{path} not rendered"));
        String::from_utf8(f.bytes.clone()).unwrap()
    }

    #[test]
    fn agents_md_links_to_install_dir() {
        let (tmp, skills) = fixture();
        let s = render_one(&skills, Format::AgentsMd, tmp.path(), "AGENTS.md");
        assert!(
            s.contains("- **demo** — Demo skill. (`.agents/skills/demo/SKILL.md`)"),
            "{s}"
        );
        let g = render_one(&skills, Format::Gemini, tmp.path(), "GEMINI.md");
        assert_eq!(g, s);
    }

    #[test]
    fn bundle_has_toc_and_bodies() {
        let (tmp, skills) = fixture();
        let s = render_one(&skills, Format::Bundle, tmp.path(), "SKILLS.md");
        assert!(s.contains("- [demo](#demo)\n"));
        assert!(s.contains("## demo\n\n> Demo skill.\n\n# demo\nBody of demo.\n"));
        assert!(!s.contains("name: demo"), "frontmatter must not leak");
        let a = render_one(&skills, Format::Aider, tmp.path(), "CONVENTIONS.md");
        assert_eq!(a, s);
    }

    #[test]
    fn rule_formats_frontmatter() {
        let (tmp, skills) = fixture();
        let out = tmp.path();
        assert_eq!(
            render_one(&skills, Format::Cursor, out, ".cursor/rules/demo.mdc"),
            "---\ndescription: Demo skill.\nglobs:\nalwaysApply: false\n---\n\n> Reference files mentioned below are in `.agents/skills/demo/references/` (install with `orchnes install --target agents`).\n\n# demo\nBody of demo.\n"
        );
        assert!(
            render_one(
                &skills,
                Format::Cursor,
                out,
                ".cursor/rules/agent-behavior.mdc"
            )
            .contains("alwaysApply: true")
        );
        assert!(
            render_one(&skills, Format::Windsurf, out, ".windsurf/rules/demo.md")
                .starts_with("---\ntrigger: model_decision\ndescription: Demo skill.\n---\n")
        );
        assert!(
            render_one(
                &skills,
                Format::Windsurf,
                out,
                ".windsurf/rules/agent-behavior.md"
            )
            .contains("trigger: always_on")
        );
        assert!(
            render_one(&skills, Format::Continue, out, ".continue/rules/demo.md").starts_with(
                "---\nname: demo\ndescription: Demo skill.\nalwaysApply: false\n---\n"
            )
        );
    }

    #[test]
    fn copilot_files() {
        let (tmp, skills) = fixture();
        let out = tmp.path();
        let main = render_one(
            &skills,
            Format::Copilot,
            out,
            ".github/copilot-instructions.md",
        );
        assert!(main.contains("(`.github/skills/demo/SKILL.md`)"));
        assert!(main.contains("## agent-behavior\n\n# agent-behavior\nBody of agent-behavior.\n"));
        assert!(!main.starts_with("---"));
        assert_eq!(
            render(&skills, Format::Copilot).unwrap().len(),
            1,
            "index only"
        );
    }

    #[test]
    fn claude_zip_contains_skill_folder() {
        let (tmp, skills) = fixture();
        let out = tmp.path().join("zips");
        let written = export(&skills, Format::ClaudeZip, &out).unwrap();
        assert_eq!(written.len(), 2);
        let mut archive =
            zip::ZipArchive::new(fs::File::open(out.join("demo.zip")).unwrap()).unwrap();
        let mut names: Vec<String> = archive
            .file_names()
            .map(|n| n.unwrap().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["demo/SKILL.md", "demo/references/r.md"]);
        let mut body = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("demo/SKILL.md").unwrap(), &mut body)
            .unwrap();
        assert!(body.starts_with("---\nname: demo\n"));
    }
}
