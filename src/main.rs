use agent_orchestrator::export::{self, Format};
use agent_orchestrator::install::{self, Scope, Target};
use agent_orchestrator::routing::{self, Factors, SENSITIVE_NOTE};
use agent_orchestrator::skill::{self, Skill};
use agent_orchestrator::validate;
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Load, validate, route and export Agent Skills.
#[derive(Parser)]
#[command(name = "skillctl", version)]
struct Cli {
    /// Folder containing one subfolder per skill.
    #[arg(long, global = true, default_value = "skills")]
    skills_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List skills: name and first sentence of the description.
    List,
    /// Print a skill's SKILL.md.
    Show { name: String },
    /// Check every skill like agent-skills' scripts/validate.py; exit 1 on errors.
    Validate,
    /// Pick a model tier with the agent-orchestra routing matrix (factors 1–3).
    Route {
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
        complexity: u8,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
        risk: u8,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
        context: u8,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
        capability: u8,
        /// Task handles privacy-sensitive data.
        #[arg(long)]
        sensitive: bool,
    },
    /// Write skills in another agent's instruction format.
    Export {
        #[arg(long, value_enum)]
        format: Format,
        #[arg(long)]
        out: PathBuf,
    },
    /// Copy skill folders into an agent's native skills directory.
    Install {
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long, value_enum, default_value = "project")]
        scope: Scope,
        /// Base directory (default: current dir for project scope, home for user scope).
        #[arg(long)]
        dest: Option<PathBuf>,
        /// Replace skill folders that already exist at the destination.
        #[arg(long)]
        force: bool,
        /// Skills to install (default: all).
        names: Vec<String>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let dir = &cli.skills_dir;
    match cli.command {
        Command::List => {
            for s in load_all(dir)? {
                println!(
                    "{:<26} {}",
                    s.name(),
                    skill::first_sentence(s.description())
                );
            }
        }
        Command::Show { name } => print!("{}", load_one(dir, &name)?.raw),
        Command::Validate => {
            let report = validate::validate(dir)?;
            for e in &report.errors {
                println!("ERROR {e}");
            }
            println!(
                "{} skills checked, {} errors",
                report.checked,
                report.errors.len()
            );
            if !report.errors.is_empty() {
                return Ok(ExitCode::FAILURE);
            }
        }
        Command::Route {
            complexity,
            risk,
            context,
            capability,
            sensitive,
        } => {
            let f = Factors {
                complexity,
                risk,
                context,
                capability,
            };
            let r = routing::route(f, sensitive)?;
            println!("tier: {}\nreason: {}", r.tier, r.reason);
            if r.local_only {
                println!("providers: {SENSITIVE_NOTE}");
            }
        }
        Command::Export { format, out } => {
            let written = export::export(&load_all(dir)?, format, &out)?;
            println!("wrote {} files to {}", written.len(), out.display());
        }
        Command::Install {
            target,
            scope,
            dest,
            force,
            names,
        } => {
            let skills = if names.is_empty() {
                load_all(dir)?
            } else {
                names
                    .iter()
                    .map(|n| load_one(dir, n))
                    .collect::<Result<_>>()?
            };
            let base = match (dest, scope) {
                (Some(d), _) => d,
                (None, Scope::Project) => PathBuf::from("."),
                (None, Scope::User) => home()?,
            };
            let dest = base.join(install::skills_subdir(target, scope));
            let installed = install::install(&skills, &dest, force)?;
            println!("installed {} skills to {}", installed.len(), dest.display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn load_all(dir: &Path) -> Result<Vec<Skill>> {
    skill::load_all(dir).with_context(|| format!("loading skills from {}", dir.display()))
}

fn load_one(dir: &Path, name: &str) -> Result<Skill> {
    if !skill::is_valid_name(name) {
        bail!("invalid skill name '{name}'");
    }
    Ok(skill::load_skill(&dir.join(name))?)
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .context("cannot find home directory (set HOME or pass --dest)")
}
