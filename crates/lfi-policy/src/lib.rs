//! `lfi-policy` — NeuPSL (Probabilistic Soft Logic) DSL.
//!
//! Typed DSL for declaring weighted policy rules. A policy
//! library is a collection of `Rule`s; each rule fires with a
//! strength when its body's atoms hold. Rules can be soft
//! (weighted) or hard (must hold).
//!
//! ## Why typed
//!
//! Strings are not policy. Hand-rolled regex or LLM prompts
//! can't be replayed, audited, or proven sound. A typed DSL
//! gives:
//!
//! - **Replayability**: same proposal → same decision (modulo
//!   corpus evolution)
//! - **Auditability**: every Decision traces back to specific
//!   `RuleId`s the operator can read
//! - **Testability**: the policy library is a function of its
//!   atoms; you write tests against it
//!
//! ## Status
//!
//! Typed surface scaffold. The actual NeuPSL solver lands in
//! a follow-up — this crate ships the types so downstream
//! consumers (lfi-critic, Forge-LFI) can author rule libraries
//! before the solver is finished.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use lfi_core::{RuleId, Strength};
use serde::{Deserialize, Serialize};

pub mod canonical;

/// An atomic predicate that holds (or doesn't) of a proposal.
///
/// Atoms are intentionally opaque at this layer — the
/// application-specific Critic decides what `name` and `args`
/// mean for its proposal shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Atom {
    /// Predicate name (e.g. `has-h1`, `passes-contrast`).
    pub name: String,
    /// Predicate arguments (e.g. `["section-1"]`).
    #[serde(default)]
    pub args: Vec<String>,
}

/// Negation polarity of an atom in a rule body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Polarity {
    /// Atom must hold.
    Positive,
    /// Atom must NOT hold.
    Negative,
}

/// One literal in a rule body — a polarised atom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Literal {
    /// The atom.
    pub atom: Atom,
    /// Whether the atom must hold or must NOT hold.
    pub polarity: Polarity,
}

/// Rule hardness — soft rules have weights, hard rules MUST
/// hold for the proposal to be acceptable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hardness {
    /// Hard constraint — violation = immediate reject.
    Hard,
    /// Soft constraint — violation contributes weighted
    /// penalty to the aggregate decision.
    Soft,
}

/// One policy rule.
///
/// Reads informally as "if every literal in `body` holds with
/// its declared polarity, then `head` holds with `weight`."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Rule {
    /// Stable rule identifier (kebab-case slug).
    pub id: RuleId,
    /// One-line human description.
    pub description: String,
    /// Rule hardness.
    pub hardness: Hardness,
    /// Weight for soft rules (ignored for hard).
    pub weight: Strength,
    /// Body literals — conjunction.
    pub body: Vec<Literal>,
    /// Head atom — what this rule concludes.
    pub head: Atom,
}

/// A policy library — a named collection of rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PolicyLibrary {
    /// Library name (e.g. `forge-default`, `tenant-acme`).
    pub name: String,
    /// Library version — bump when rules change semantically.
    pub version: String,
    /// The rules.
    pub rules: Vec<Rule>,
}

impl PolicyLibrary {
    /// Construct an empty library.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            rules: Vec::new(),
        }
    }
    /// Count of hard rules.
    pub fn hard_count(&self) -> usize {
        self.rules
            .iter()
            .filter(|r| r.hardness == Hardness::Hard)
            .count()
    }
    /// Count of soft rules.
    pub fn soft_count(&self) -> usize {
        self.rules
            .iter()
            .filter(|r| r.hardness == Hardness::Soft)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_library() {
        let l = PolicyLibrary::new("test", "0.1");
        assert_eq!(l.hard_count(), 0);
        assert_eq!(l.soft_count(), 0);
    }

    #[test]
    fn rule_serde_roundtrip() {
        let r = Rule {
            id: RuleId::new("test-rule"),
            description: "test".into(),
            hardness: Hardness::Hard,
            weight: Strength::FULL,
            body: vec![Literal {
                atom: Atom {
                    name: "has-h1".into(),
                    args: vec![],
                },
                polarity: Polarity::Positive,
            }],
            head: Atom {
                name: "page-valid".into(),
                args: vec![],
            },
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: Rule = serde_json::from_str(&j).unwrap();
        assert_eq!(r, back);
    }
}
