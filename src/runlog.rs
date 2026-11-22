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
/// `YYYY-MM-DDTHH:MMZ` in UTC, the run-log timestamp format.
pub fn utc_timestamp(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60
    )
}

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

    #[test]
    fn utc_timestamps() {
        use std::time::{Duration, UNIX_EPOCH};
        let at = |s| utc_timestamp(UNIX_EPOCH + Duration::from_secs(s));
        assert_eq!(at(0), "1970-01-01T00:00Z");
        assert_eq!(at(1_700_000_000), "2023-11-14T22:13Z");
        assert_eq!(at(951_825_600), "2000-02-29T12:00Z");
    }
}
