//! Task contract and review verdict (agent-orchestra `references/contracts.md`).

use crate::routing::Tier;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Research,
    Build,
    Test,
    Security,
    Docs,
    Content,
    Data,
}

/// Orchestrator → specialist. No contract, no dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskContract {
    pub id: String,
    pub role: Role,
    pub tier: Tier,
    pub goal: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    pub acceptance: Vec<String>,
    pub output: String,
    pub size_budget: Option<String>,
    #[serde(default)]
    pub playbooks: Vec<String>,
    pub budget: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum VerdictKind {
    Accept,
    Revise,
    Reassign,
}

/// Reviewer → orchestrator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub task: String,
    pub verdict: VerdictKind,
    pub evidence: String,
    #[serde(default)]
    pub issues: Vec<String>,
    pub bloat: Option<String>,
    pub teach: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid YAML: {0}")]
pub struct ContractError(#[from] serde_norway::Error);

impl TaskContract {
    pub fn from_yaml(s: &str) -> Result<Self, ContractError> {
        Ok(serde_norway::from_str(s)?)
    }
    pub fn to_yaml(&self) -> Result<String, ContractError> {
        Ok(serde_norway::to_string(self)?)
    }
}

impl Verdict {
    pub fn from_yaml(s: &str) -> Result<Self, ContractError> {
        Ok(serde_norway::from_str(s)?)
    }
    pub fn to_yaml(&self) -> Result<String, ContractError> {
        Ok(serde_norway::to_string(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example from contracts.md. Two items containing `: ` are quoted here: unquoted,
    /// YAML reads `key: value` inside a list item as a mapping, not a string.
    const EXAMPLE: &str = r#"
id: TASK-7
role: build            # research | build | test | security | docs | content | data
tier: T2
goal: Add VAT calculation to checkout totals
inputs:
  - src/checkout/total.rs (lines 1-80)
  - "decision D4: VAT rates come from config, not hardcoded"
constraints:
  - Rust, no new dependencies
  - do not touch src/payments/**
acceptance:
  - "cargo test checkout:: passes"
  - new tests cover 0%, 10%, 20% rates and rounding to 2 decimals
output: unified diff + test output
size_budget: ~60 changed lines
playbooks: [rust-money-handling]
budget: 30k tokens
"#;

    #[test]
    fn parses_contracts_md_example_and_round_trips() {
        let c = TaskContract::from_yaml(EXAMPLE).unwrap();
        assert_eq!(c.id, "TASK-7");
        assert_eq!(c.role, Role::Build);
        assert_eq!(c.tier, Tier::T2);
        assert_eq!(c.inputs.len(), 2);
        assert_eq!(c.playbooks, vec!["rust-money-handling"]);
        assert_eq!(c.budget.as_deref(), Some("30k tokens"));
        let back = TaskContract::from_yaml(&c.to_yaml().unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn rejects_unknown_role() {
        let bad = EXAMPLE.replace("role: build ", "role: chat ");
        assert!(TaskContract::from_yaml(&bad).is_err());
    }

    #[test]
    fn verdict_round_trip() {
        let v = Verdict {
            task: "TASK-7".into(),
            verdict: VerdictKind::Revise,
            evidence: "cargo test: 11/12 passed".into(),
            issues: vec!["src/checkout/total.rs:42 rounds half-down".into()],
            bloat: Some("unused Currency trait".into()),
            teach: None,
        };
        let yaml = v.to_yaml().unwrap();
        assert!(yaml.contains("verdict: REVISE"), "{yaml}");
        assert_eq!(Verdict::from_yaml(&yaml).unwrap(), v);
    }
}
