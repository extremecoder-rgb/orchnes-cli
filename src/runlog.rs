//! Append-only run log (agent-orchestra `references/contracts.md`). One entry per line:
//!
//! ```text
//! 2026-11-21T14:32Z | TASK-7 | build | model=<id> T2 | REVISE→ACCEPT | tests 12/12 | 18.4k tok | $0.04 | why T2: standard feature, low risk
//! ```

use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

const SEP: &str = " | ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub timestamp: String,
    pub task: String,
    pub role: String,
    pub model: String,
    pub tier: String,
    /// Verdict history, e.g. `REVISE→ACCEPT`.
    pub verdicts: String,
    pub evidence: String,
    pub tokens: String,
    pub cost: String,
    /// Decision rationale, written after `why `.
    pub why: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RunLogError {
    #[error("field contains ' | ' or a newline: {0:?}")]
    BadField(String),
    #[error("malformed run log line: {0}")]
    Malformed(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} | {} | {} | model={} {} | {} | {} | {} | {} | why {}",
            self.timestamp,
            self.task,
            self.role,
            self.model,
            self.tier,
            self.verdicts,
            self.evidence,
            self.tokens,
            self.cost,
            self.why
        )
    }
}

impl FromStr for Entry {
    type Err = RunLogError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let bad = || RunLogError::Malformed(line.to_string());
        let parts: Vec<&str> = line.splitn(9, SEP).collect();
        let [
            ts,
            task,
            role,
            model_tier,
            verdicts,
            evidence,
            tokens,
            cost,
            why,
        ] = parts[..]
        else {
            return Err(bad());
        };
        let (model, tier) = model_tier
            .strip_prefix("model=")
            .and_then(|m| m.rsplit_once(' '))
            .ok_or_else(bad)?;
        let why = why.strip_prefix("why ").ok_or_else(bad)?;
        Ok(Entry {
            timestamp: ts.into(),
            task: task.into(),
            role: role.into(),
            model: model.into(),
            tier: tier.into(),
            verdicts: verdicts.into(),
            evidence: evidence.into(),
            tokens: tokens.into(),
            cost: cost.into(),
            why: why.into(),
        })
    }
}

impl Entry {
    fn check(&self) -> Result<(), RunLogError> {
        let fields = [
            &self.timestamp,
            &self.task,
            &self.role,
            &self.model,
            &self.tier,
            &self.verdicts,
            &self.evidence,
            &self.tokens,
            &self.cost,
            &self.why,
        ];
        match fields
            .iter()
            .find(|f| f.contains(SEP) || f.contains('\n') || f.contains('\r'))
        {
            Some(f) => Err(RunLogError::BadField(f.to_string())),
            None if self.model.contains(' ') => Err(RunLogError::BadField(self.model.clone())),
            None => Ok(()),
        }
    }
}

/// Appends one line; never rewrites existing lines.
pub fn append(path: &Path, entry: &Entry) -> Result<(), RunLogError> {
    entry.check()?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{entry}")?;
    Ok(())
}

/// Reads every entry; fails on the first malformed non-empty line.
pub fn read(path: &Path) -> Result<Vec<Entry>, RunLogError> {
    std::fs::read_to_string(path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::parse)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "2026-11-21T14:32Z | TASK-7 | build | model=<id> T2 | REVISE→ACCEPT | tests 12/12 | 18.4k tok | $0.04 | why T2: standard feature, low risk";

    #[test]
    fn parses_and_formats_contracts_md_example() {
        let e: Entry = EXAMPLE.parse().unwrap();
        assert_eq!(e.model, "<id>");
        assert_eq!(e.tier, "T2");
        assert_eq!(e.verdicts, "REVISE→ACCEPT");
        assert_eq!(e.why, "T2: standard feature, low risk");
        assert_eq!(e.to_string(), EXAMPLE);
    }

    #[test]
    fn rejects_malformed() {
        assert!("a | b".parse::<Entry>().is_err());
        assert!(EXAMPLE.replace("model=", "m=").parse::<Entry>().is_err());
    }

    #[test]
    fn appends_without_rewriting() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("run.log");
        let first: Entry = EXAMPLE.parse().unwrap();
        let mut second = first.clone();
        second.task = "TASK-8".into();
        append(&path, &first).unwrap();
        append(&path, &second).unwrap();
        assert_eq!(read(&path).unwrap(), vec![first.clone(), second]);

        let mut bad = first;
        bad.evidence = "a | b".into();
        assert!(matches!(append(&path, &bad), Err(RunLogError::BadField(_))));
        assert_eq!(read(&path).unwrap().len(), 2);
    }
}
