//! `lfi-critic` — the `Critic` trait + reference implementations.
//!
//! The **`Critic` trait** is the compiler-enforced seam: every
//! `Proposal` flows through `Critic::evaluate -> Decision`
//! before any commit. The platform's commit boundary accepts
//! only `Decision`, never a raw `Proposal` payload — that's a
//! type signature, not a code-review policy.
//!
//! ## Implementations shipped
//!
//! - **`NoopCritic`** — always returns `Accept`. Used for
//!   non-AI pipelines (operator-authored content goes through
//!   the same call shape as LLM proposals, just with a
//!   trivial Critic).
//! - **`LfiCritic`** — reference implementation that
//!   evaluates against a `PolicyLibrary` + an optional
//!   `Corpus`. Currently a typed skeleton — solver wiring
//!   lands in a follow-up.
//!
//! Downstream application forks (e.g. Forge-LFI) provide
//! their own Critic impls tailored to their proposal shape.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use lfi_core::{Decision, Proposal, Strength};
use lfi_corpus::Corpus;
use lfi_policy::PolicyLibrary;

/// The Critic trait. The seam.
///
/// Implementations evaluate a `Proposal` and return a typed
/// `Decision`. Every call site that wants to commit a proposal
/// must hold a `&dyn Critic` and call `.evaluate()` first.
pub trait Critic: Send + Sync {
    /// Evaluate the proposal. Pure function in the
    /// computational sense — given the same proposal +
    /// internal state, the same Decision should result. (May
    /// observe internal state — corpora evolve — but should
    /// not have observable side effects on the proposal.)
    fn evaluate(&self, proposal: &Proposal) -> Decision;

    /// A short identifier for this Critic. Used in audit
    /// logs so consumers can trace which Critic produced a
    /// Decision.
    fn ident(&self) -> &'static str;
}

/// Always-accept Critic. Used for pipelines that don't have
/// AI generation at all — operator-authored content still
/// flows through the seam to keep the call shape uniform.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCritic;

impl Critic for NoopCritic {
    fn evaluate(&self, _proposal: &Proposal) -> Decision {
        Decision::Accept {
            confidence: Strength::FULL,
            traced_rules_fired: vec![],
        }
    }
    fn ident(&self) -> &'static str {
        "noop"
    }
}

/// Reference LFI Critic. Evaluates against a typed policy
/// library + an optional corpus.
///
/// The Critic stores its policy + corpus by value. To swap
/// policies at runtime, swap the LfiCritic instance behind
/// the `dyn Critic`.
#[derive(Debug)]
pub struct LfiCritic {
    /// The policy library this Critic evaluates against.
    pub policy: PolicyLibrary,
    /// Optional curated corpus for originality / similarity
    /// checks.
    pub corpus: Option<Corpus>,
}

impl LfiCritic {
    /// Construct an LfiCritic from a policy library only
    /// (no corpus checks).
    pub fn from_policy(policy: PolicyLibrary) -> Self {
        Self {
            policy,
            corpus: None,
        }
    }

    /// Attach a corpus for similarity/originality checks.
    pub fn with_corpus(mut self, corpus: Corpus) -> Self {
        self.corpus = Some(corpus);
        self
    }
}

impl Critic for LfiCritic {
    fn evaluate(&self, _proposal: &Proposal) -> Decision {
        // SCAFFOLD: the actual NeuPSL solve + HDC similarity
        // pass lands when paul's downstream LFI session
        // wires the Neurosymbolic-Toolkit substrates. Until
        // then we return Accept with traced library identity
        // so call sites can be wired + tested end-to-end.
        Decision::Accept {
            confidence: Strength::new(0.5),
            traced_rules_fired: vec![],
        }
    }
    fn ident(&self) -> &'static str {
        "lfi-reference"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lfi_core::Proposal;

    #[test]
    fn noop_accepts_anything() {
        let c = NoopCritic;
        let p = Proposal {
            kind: "test".into(),
            payload: serde_json::json!({}),
            context: Default::default(),
        };
        let d = c.evaluate(&p);
        assert!(d.is_accept());
        assert_eq!(c.ident(), "noop");
    }

    #[test]
    fn lfi_critic_scaffold_accepts() {
        let c = LfiCritic::from_policy(PolicyLibrary::new("test", "0.1"));
        let p = Proposal {
            kind: "test".into(),
            payload: serde_json::json!({}),
            context: Default::default(),
        };
        assert!(c.evaluate(&p).is_accept());
        assert_eq!(c.ident(), "lfi-reference");
    }

    /// Sanity: `dyn Critic` works for swap-at-runtime.
    #[test]
    fn dyn_dispatch_works() {
        let critics: Vec<Box<dyn Critic>> = vec![
            Box::new(NoopCritic),
            Box::new(LfiCritic::from_policy(PolicyLibrary::new("test", "0.1"))),
        ];
        let p = Proposal {
            kind: "test".into(),
            payload: serde_json::json!({}),
            context: Default::default(),
        };
        for c in &critics {
            assert!(c.evaluate(&p).is_accept());
        }
    }
}
