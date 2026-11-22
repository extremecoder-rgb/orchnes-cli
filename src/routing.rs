//! The agent-orchestra routing matrix (`references/routing-matrix.md`).
//!
//! Each factor is scored 1–3. Any factor = 3 → T1. Otherwise (sum 4–8):
//! sum ≤ 6 → T3, 7–8 → T2. Privacy-sensitive work is restricted to local or
//! compliant providers regardless of tier.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Model tier: T1 top-ranked, T2 strong mid-cost, T3 small/free/local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    T1,
    T2,
    T3,
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Factors {
    pub complexity: u8,
    pub risk: u8,
    pub context: u8,
    pub capability: u8,
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("{factor} must be 1, 2 or 3 (got {value})")]
pub struct FactorError {
    pub factor: &'static str,
    pub value: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub tier: Tier,
    pub reason: String,
    /// True when the task must stay on local or compliant providers.
    pub local_only: bool,
}

pub const SENSITIVE_NOTE: &str = "local/compliant providers only";

pub fn route(f: Factors, sensitive: bool) -> Result<Route, FactorError> {
    let named = [
        ("complexity", f.complexity),
        ("risk", f.risk),
        ("context", f.context),
        ("capability", f.capability),
    ];
    if let Some(&(factor, value)) = named.iter().find(|(_, v)| !(1..=3).contains(v)) {
        return Err(FactorError { factor, value });
    }
    let maxed: Vec<&str> = named
        .iter()
        .filter(|(_, v)| *v == 3)
        .map(|(n, _)| *n)
        .collect();
    let sum: u8 = named.iter().map(|(_, v)| v).sum();
    let (tier, reason) = if !maxed.is_empty() {
        (
            Tier::T1,
            format!("{} = 3 (any factor 3 → T1)", maxed.join(", ")),
        )
    } else if sum <= 6 {
        (Tier::T3, format!("all factors ≤ 2, sum {sum} (≤ 6 → T3)"))
    } else {
        (Tier::T2, format!("all factors ≤ 2, sum {sum} (7–8 → T2)"))
    };
    Ok(Route {
        tier,
        reason,
        local_only: sensitive,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(c: u8, r: u8, x: u8, k: u8) -> Tier {
        let f = Factors {
            complexity: c,
            risk: r,
            context: x,
            capability: k,
        };
        route(f, false).unwrap().tier
    }

    #[test]
    fn matrix_over_every_combination() {
        for c in 1..=3 {
            for r in 1..=3 {
                for x in 1..=3 {
                    for k in 1..=3 {
                        let expected = if [c, r, x, k].contains(&3) {
                            Tier::T1
                        } else if c + r + x + k <= 6 {
                            Tier::T3
                        } else {
                            Tier::T2
                        };
                        assert_eq!(tier(c, r, x, k), expected, "{c}{r}{x}{k}");
                    }
                }
            }
        }
    }

    #[test]
    fn boundaries() {
        assert_eq!(tier(1, 1, 1, 1), Tier::T3);
        assert_eq!(tier(2, 2, 1, 1), Tier::T3);
        assert_eq!(tier(2, 2, 2, 1), Tier::T2);
        assert_eq!(tier(2, 2, 2, 2), Tier::T2);
        assert_eq!(tier(1, 3, 1, 1), Tier::T1);
    }

    #[test]
    fn reason_and_sensitivity() {
        let f = Factors {
            complexity: 3,
            risk: 1,
            context: 3,
            capability: 1,
        };
        let r = route(f, true).unwrap();
        assert_eq!(r.reason, "complexity, context = 3 (any factor 3 → T1)");
        assert!(r.local_only);
    }

    #[test]
    fn rejects_out_of_range() {
        let f = Factors {
            complexity: 1,
            risk: 4,
            context: 1,
            capability: 1,
        };
        assert_eq!(
            route(f, false).unwrap_err().to_string(),
            "risk must be 1, 2 or 3 (got 4)"
        );
    }
}
