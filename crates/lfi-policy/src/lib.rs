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

    /// Validate library invariants. Returns the list of every
    /// violation found — empty Vec means the library is sound.
    ///
    /// Checks (in stable order):
    ///
    /// 1. Library name is non-empty.
    /// 2. Library version is non-empty.
    /// 3. No two rules share a `RuleId` (rule IDs are the audit
    ///    log's primary key — duplicates would make a Decision's
    ///    `traced_rules_fired` ambiguous).
    /// 4. Every rule has at least one body literal (a rule with
    ///    no body fires unconditionally — almost always an
    ///    author bug; require `head` to be declared as a
    ///    standalone Fact if that's the intent).
    /// 5. Soft rules carry a non-zero weight (a Soft rule with
    ///    `Strength::NONE` is a no-op; flag as author error).
    /// 6. Every Atom has a non-empty name.
    pub fn validate(&self) -> Vec<PolicyValidationError> {
        let mut errors = Vec::new();
        if self.name.is_empty() {
            errors.push(PolicyValidationError::EmptyLibraryName);
        }
        if self.version.is_empty() {
            errors.push(PolicyValidationError::EmptyLibraryVersion);
        }
        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for rule in &self.rules {
            let id_slug = rule.id.as_str().to_owned();
            if !seen_ids.insert(id_slug.clone()) {
                errors.push(PolicyValidationError::DuplicateRuleId(id_slug.clone()));
            }
            if rule.body.is_empty() {
                errors.push(PolicyValidationError::EmptyRuleBody(id_slug.clone()));
            }
            if rule.hardness == Hardness::Soft && rule.weight == Strength::NONE {
                errors.push(PolicyValidationError::SoftRuleZeroWeight(id_slug.clone()));
            }
            if rule.head.name.is_empty() {
                errors.push(PolicyValidationError::EmptyAtomName(id_slug.clone()));
            }
            for lit in &rule.body {
                if lit.atom.name.is_empty() {
                    errors.push(PolicyValidationError::EmptyAtomName(id_slug.clone()));
                }
            }
        }
        errors
    }

    /// True iff `validate()` returns no errors.
    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

/// Validation errors returned by [`PolicyLibrary::validate`].
///
/// Each variant carries the offending rule's id (when applicable)
/// so the operator can find the bad rule by slug grep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyValidationError {
    /// `library.name` is the empty string.
    EmptyLibraryName,
    /// `library.version` is the empty string.
    EmptyLibraryVersion,
    /// Two rules share this `RuleId`.
    DuplicateRuleId(String),
    /// This rule's body has zero literals (fires unconditionally).
    EmptyRuleBody(String),
    /// A Soft rule with `Strength::NONE` is a no-op.
    SoftRuleZeroWeight(String),
    /// An Atom in this rule has an empty name.
    EmptyAtomName(String),
}

impl std::fmt::Display for PolicyValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyLibraryName => write!(f, "library.name is empty"),
            Self::EmptyLibraryVersion => write!(f, "library.version is empty"),
            Self::DuplicateRuleId(id) => write!(f, "duplicate rule id: {id}"),
            Self::EmptyRuleBody(id) => write!(f, "rule {id} has empty body"),
            Self::SoftRuleZeroWeight(id) => {
                write!(f, "soft rule {id} has zero weight (no-op)")
            }
            Self::EmptyAtomName(id) => write!(f, "rule {id} has an atom with empty name"),
        }
    }
}

impl std::error::Error for PolicyValidationError {}

/// Fluent builder for [`Rule`]. Construct via [`Rule::builder`].
///
/// Less noisy than a struct literal; lets `canonical.rs` and
/// downstream policy authors write rules in a single chained
/// expression with sensible defaults (Hard hardness, FULL weight,
/// empty body).
#[derive(Debug, Clone)]
pub struct RuleBuilder {
    id: RuleId,
    description: String,
    hardness: Hardness,
    weight: Strength,
    body: Vec<Literal>,
    head: Atom,
}

impl Rule {
    /// Begin building a Rule. Defaults: Hard hardness, FULL
    /// weight, empty body, head atom = `acceptable`.
    pub fn builder(id: impl Into<String>) -> RuleBuilder {
        RuleBuilder {
            id: RuleId::new(id),
            description: String::new(),
            hardness: Hardness::Hard,
            weight: Strength::FULL,
            body: Vec::new(),
            head: Atom {
                name: "acceptable".to_owned(),
                args: Vec::new(),
            },
        }
    }
}

impl RuleBuilder {
    /// One-line human description.
    pub fn description(mut self, s: impl Into<String>) -> Self {
        self.description = s.into();
        self
    }
    /// Mark this rule as Soft + set the weight.
    pub fn soft(mut self, weight: Strength) -> Self {
        self.hardness = Hardness::Soft;
        self.weight = weight;
        self
    }
    /// Mark this rule as Hard (default).
    pub fn hard(mut self) -> Self {
        self.hardness = Hardness::Hard;
        self.weight = Strength::FULL;
        self
    }
    /// Add a positive-polarity literal to the body.
    pub fn requires(mut self, atom_name: impl Into<String>, args: &[&str]) -> Self {
        self.body.push(Literal {
            atom: Atom {
                name: atom_name.into(),
                args: args.iter().map(|s| (*s).to_owned()).collect(),
            },
            polarity: Polarity::Positive,
        });
        self
    }
    /// Add a negative-polarity literal to the body.
    pub fn forbids(mut self, atom_name: impl Into<String>, args: &[&str]) -> Self {
        self.body.push(Literal {
            atom: Atom {
                name: atom_name.into(),
                args: args.iter().map(|s| (*s).to_owned()).collect(),
            },
            polarity: Polarity::Negative,
        });
        self
    }
    /// Set the head atom (default `acceptable`).
    pub fn head(mut self, atom_name: impl Into<String>, args: &[&str]) -> Self {
        self.head = Atom {
            name: atom_name.into(),
            args: args.iter().map(|s| (*s).to_owned()).collect(),
        };
        self
    }
    /// Finish — returns the constructed [`Rule`].
    pub fn build(self) -> Rule {
        Rule {
            id: self.id,
            description: self.description,
            hardness: self.hardness,
            weight: self.weight,
            body: self.body,
            head: self.head,
        }
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
    fn empty_library_passes_validation() {
        let l = PolicyLibrary::new("test", "0.1");
        assert!(l.is_valid());
    }

    #[test]
    fn library_missing_name_fails() {
        let l = PolicyLibrary::new("", "0.1");
        assert!(!l.is_valid());
        assert!(l
            .validate()
            .iter()
            .any(|e| matches!(e, PolicyValidationError::EmptyLibraryName)));
    }

    #[test]
    fn library_missing_version_fails() {
        let l = PolicyLibrary::new("test", "");
        assert!(l
            .validate()
            .iter()
            .any(|e| matches!(e, PolicyValidationError::EmptyLibraryVersion)));
    }

    #[test]
    fn duplicate_rule_id_fails_validation() {
        let mut l = PolicyLibrary::new("test", "0.1");
        l.rules.push(
            Rule::builder("dup")
                .description("first")
                .requires("a", &[])
                .build(),
        );
        l.rules.push(
            Rule::builder("dup")
                .description("second")
                .requires("b", &[])
                .build(),
        );
        let errs = l.validate();
        assert!(errs
            .iter()
            .any(|e| matches!(e, PolicyValidationError::DuplicateRuleId(id) if id == "dup")));
    }

    #[test]
    fn empty_rule_body_fails_validation() {
        let mut l = PolicyLibrary::new("test", "0.1");
        l.rules
            .push(Rule::builder("no-body").description("bare").build());
        assert!(l
            .validate()
            .iter()
            .any(|e| matches!(e, PolicyValidationError::EmptyRuleBody(id) if id == "no-body")));
    }

    #[test]
    fn soft_rule_zero_weight_fails_validation() {
        let mut l = PolicyLibrary::new("test", "0.1");
        l.rules.push(
            Rule::builder("noop-soft")
                .description("noop")
                .soft(Strength::NONE)
                .requires("a", &[])
                .build(),
        );
        assert!(l.validate().iter().any(
            |e| matches!(e, PolicyValidationError::SoftRuleZeroWeight(id) if id == "noop-soft")
        ));
    }

    #[test]
    fn canonical_forge_default_validates() {
        let l = crate::canonical::forge_default();
        let errs = l.validate();
        assert!(errs.is_empty(), "canonical library should validate; got: {errs:?}");
    }

    #[test]
    fn rule_builder_constructs_hard_rule() {
        let r = Rule::builder("test-hard")
            .description("a test")
            .requires("alpha", &["arg1"])
            .head("acceptable", &[])
            .build();
        assert_eq!(r.id.as_str(), "test-hard");
        assert_eq!(r.hardness, Hardness::Hard);
        assert_eq!(r.weight, Strength::FULL);
        assert_eq!(r.body.len(), 1);
        assert_eq!(r.body[0].polarity, Polarity::Positive);
    }

    #[test]
    fn rule_builder_forbids_adds_negative_literal() {
        let r = Rule::builder("test")
            .forbids("bad-thing", &[])
            .build();
        assert_eq!(r.body[0].polarity, Polarity::Negative);
    }

    #[test]
    fn rule_builder_soft_sets_weight_and_hardness() {
        let r = Rule::builder("test-soft")
            .soft(Strength::new(0.7))
            .requires("x", &[])
            .build();
        assert_eq!(r.hardness, Hardness::Soft);
        assert_eq!(r.weight, Strength::new(0.7));
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
