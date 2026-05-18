//! `lfi-core` — typed core for the Lattice of Formal Inference.
//!
//! This crate is the **typed transport** between application
//! code and the Neurosymbolic-Toolkit substrates. It exposes a
//! small surface — `Proposal`, `Decision`, `RuleId`, `Strength`,
//! `Violation` — that every downstream crate (policy, corpus,
//! critic, application adapters) agrees on.
//!
//! ## What lives here
//!
//! - Typed data carrying a proposal awaiting evaluation
//! - Typed decision returned by a Critic
//! - Stable rule identifiers (kebab-case slugs)
//! - Soft-logic strength values (0.0..=1.0)
//!
//! ## What does NOT live here
//!
//! - The `Critic` trait — that's in `lfi-critic`
//! - NeuPSL DSL — that's in `lfi-policy`
//! - HDC corpora — that's in `lfi-corpus`
//! - Application-specific proposal kinds — those are in
//!   downstream forks (e.g. Forge-LFI defines `Cms` /
//!   `BlockShape` / etc.)
//!
//! ## Neurosymbolic-Toolkit
//!
//! Per task #34: this crate **adds the toolkit as a Cargo
//! dep** but does not modify the toolkit. Downstream crates
//! (lfi-policy, lfi-corpus, lfi-critic) consume specific
//! sub-crates (`hdc-core`, `neupsl`, `lnn`, `vsa`, `hdlm`,
//! `math-codec`) via the re-export here.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};

/// Stable rule identifier. Kebab-case slug from the policy
/// library (e.g. `wcag-contrast`, `brand-voice-passive-cap`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuleId(String);

impl RuleId {
    /// Construct from any kebab-case slug.
    pub fn new(slug: impl Into<String>) -> Self {
        Self(slug.into())
    }
    /// Slug as &str.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Probabilistic soft-logic strength, clamped to [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Strength(f32);

impl Strength {
    /// Construct from any f32, clamped to [0.0, 1.0].
    pub fn new(v: f32) -> Self {
        Self(v.clamp(0.0, 1.0))
    }
    /// 1.0 — strongest possible.
    pub const FULL: Self = Self(1.0);
    /// 0.0 — weakest possible.
    pub const NONE: Self = Self(0.0);
    /// Get the inner f32.
    pub fn get(&self) -> f32 {
        self.0
    }
}

/// Proposal source — who or what originated this proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProposalSource {
    /// A human operator authored or edited this proposal.
    #[default]
    Operator,
    /// A large language model produced this proposal as a
    /// candidate. MUST flow through the Critic.
    Llm,
    /// A multi-stage pipeline produced this proposal (e.g.
    /// IA → Wireframe → Content → Audit).
    Pipeline,
    /// Imported from an external source (RSS, sitemap,
    /// upstream CMS, etc.).
    Imported,
    /// An autonomous agent action.
    Agent,
}

/// Context attached to a proposal for evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ProposalContext {
    /// Tenant identifier — for multi-tenant policy isolation.
    pub tenant_id: Option<String>,
    /// User identifier — for per-user policy slices.
    pub user_id: Option<String>,
    /// UI surface the proposal targets ("landing", "blog
    /// post", "checkout", etc.) — keys policy rules.
    pub surface: Option<String>,
    /// Where this proposal came from.
    pub source: ProposalSource,
}

/// A proposal awaiting evaluation.
///
/// The `payload` is intentionally `serde_json::Value` so that
/// the same Critic trait works across heterogeneous proposal
/// shapes. Downstream forks define their own typed payload
/// schemas and rehydrate from this.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Proposal {
    /// Kebab-case kind slug (e.g. `cms-section`,
    /// `design-tokens`, `meta-description`).
    pub kind: String,
    /// Proposal payload — opaque to lfi-core; the downstream
    /// Critic deserializes.
    pub payload: serde_json::Value,
    /// Evaluation context.
    #[serde(default)]
    pub context: ProposalContext,
}

/// One violated rule with its strength + a human-readable
/// explanation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Violation {
    /// Rule identifier.
    pub rule: RuleId,
    /// How strongly the rule fired.
    pub strength: Strength,
    /// Operator-facing explanation. Should reference specific
    /// payload coordinates when possible.
    pub explanation: String,
}

/// The decision returned by a Critic.
///
/// Three variants by design:
///
/// - **Accept** with traced rules → ship it
/// - **Reject** with violations → don't ship; rejected
/// - **Refine** with regeneration guidance → send back to the
///   generator with explicit feedback
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Decision {
    /// Proposal is accepted.
    Accept {
        /// Aggregate confidence (max strength of any
        /// supporting rule).
        confidence: Strength,
        /// Rules that fired in favor.
        traced_rules_fired: Vec<(RuleId, Strength)>,
    },
    /// Proposal is rejected; do not commit.
    Reject {
        /// One or more violated rules.
        violations: Vec<Violation>,
    },
    /// Proposal is partially acceptable; resubmit a revised
    /// version with this guidance.
    Refine {
        /// Targeted natural-language guidance for the
        /// generator (e.g. "shorten the title to under 70
        /// chars; remove the third paragraph").
        targeted_regeneration_guidance: String,
        /// Rules that motivated the refinement.
        violated_rules: Vec<Violation>,
    },
}

impl Decision {
    /// Returns true if and only if this is an `Accept`.
    pub fn is_accept(&self) -> bool {
        matches!(self, Decision::Accept { .. })
    }
    /// Returns true if and only if this is a `Reject`.
    pub fn is_reject(&self) -> bool {
        matches!(self, Decision::Reject { .. })
    }
    /// Returns true if and only if this is a `Refine`.
    pub fn is_refine(&self) -> bool {
        matches!(self, Decision::Refine { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_clamps() {
        assert_eq!(Strength::new(-0.5).get(), 0.0);
        assert_eq!(Strength::new(2.0).get(), 1.0);
        assert_eq!(Strength::new(0.3).get(), 0.3);
    }

    #[test]
    fn rule_id_roundtrip() {
        let r = RuleId::new("wcag-contrast");
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, "\"wcag-contrast\"");
        let back: RuleId = serde_json::from_str(&j).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn decision_helpers() {
        let a = Decision::Accept {
            confidence: Strength::FULL,
            traced_rules_fired: vec![],
        };
        assert!(a.is_accept() && !a.is_reject() && !a.is_refine());
        let r = Decision::Reject { violations: vec![] };
        assert!(r.is_reject());
    }

    #[test]
    fn proposal_serde_roundtrip() {
        let p = Proposal {
            kind: "cms-section".into(),
            payload: serde_json::json!({"hero": "demo"}),
            context: ProposalContext {
                tenant_id: Some("acme".into()),
                surface: Some("landing".into()),
                source: ProposalSource::Llm,
                ..Default::default()
            },
        };
        let j = serde_json::to_string(&p).unwrap();
        let back: Proposal = serde_json::from_str(&j).unwrap();
        assert_eq!(p, back);
    }
}
