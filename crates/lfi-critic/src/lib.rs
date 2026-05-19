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

use lfi_core::{Decision, Proposal, RuleId, Strength, Violation};
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

/// Always-reject Critic. Useful in tests and as a kill-switch
/// to halt a pipeline. Every proposal is rejected with the
/// same configured reason.
#[derive(Debug, Clone)]
pub struct RejectAllCritic {
    /// Reason returned in every Decision. Static so the Critic
    /// stays `Send + Sync` cheaply.
    pub reason: &'static str,
}

impl RejectAllCritic {
    /// Construct a RejectAllCritic with the given reason.
    pub const fn new(reason: &'static str) -> Self {
        Self { reason }
    }
}

impl Critic for RejectAllCritic {
    fn evaluate(&self, _proposal: &Proposal) -> Decision {
        Decision::Reject {
            violations: vec![Violation {
                rule: RuleId::new("reject-all"),
                strength: Strength::FULL,
                explanation: self.reason.to_owned(),
            }],
        }
    }
    fn ident(&self) -> &'static str {
        "reject-all"
    }
}

/// Compose multiple Critics in series. First non-Accept wins.
///
/// Use to layer cheap-and-fast checks before expensive ones, or to
/// stack independently-authored Critics into one seam. Iterates in
/// the order Critics were pushed.
#[derive(Default)]
pub struct ChainCritic {
    members: Vec<Box<dyn Critic>>,
    ident: &'static str,
}

impl ChainCritic {
    /// Construct an empty chain with the given ident.
    pub const fn new(ident: &'static str) -> Self {
        Self {
            members: Vec::new(),
            ident,
        }
    }

    /// Append a Critic to the chain. Earlier-pushed Critics run first.
    pub fn push(mut self, critic: Box<dyn Critic>) -> Self {
        self.members.push(critic);
        self
    }

    /// Number of Critics in the chain.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// True when the chain holds no Critics. An empty ChainCritic
    /// accepts everything (vacuously).
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

impl std::fmt::Debug for ChainCritic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainCritic")
            .field("ident", &self.ident)
            .field("members", &self.members.len())
            .finish()
    }
}

impl Critic for ChainCritic {
    fn evaluate(&self, proposal: &Proposal) -> Decision {
        for member in &self.members {
            let decision = member.evaluate(proposal);
            if !decision.is_accept() {
                return decision;
            }
        }
        Decision::Accept {
            confidence: Strength::FULL,
            traced_rules_fired: vec![],
        }
    }
    fn ident(&self) -> &'static str {
        self.ident
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

    #[test]
    fn reject_all_rejects_with_configured_reason() {
        let c = RejectAllCritic::new("kill-switch");
        let p = Proposal {
            kind: "test".into(),
            payload: serde_json::json!({}),
            context: Default::default(),
        };
        let d = c.evaluate(&p);
        assert!(d.is_reject());
        assert_eq!(d.violations().len(), 1);
        assert_eq!(d.violations()[0].explanation, "kill-switch");
        assert_eq!(c.ident(), "reject-all");
    }

    #[test]
    fn chain_accepts_when_empty() {
        let c = ChainCritic::new("empty-chain");
        let p = Proposal {
            kind: "test".into(),
            payload: serde_json::json!({}),
            context: Default::default(),
        };
        assert!(c.is_empty());
        assert!(c.evaluate(&p).is_accept());
    }

    #[test]
    fn chain_runs_in_order_first_rejection_wins() {
        let chain = ChainCritic::new("test-chain")
            .push(Box::new(NoopCritic))
            .push(Box::new(RejectAllCritic::new("blocked-by-policy")))
            .push(Box::new(RejectAllCritic::new("never-reached")));
        let p = Proposal {
            kind: "test".into(),
            payload: serde_json::json!({}),
            context: Default::default(),
        };
        let d = chain.evaluate(&p);
        assert!(d.is_reject());
        assert_eq!(d.violations()[0].explanation, "blocked-by-policy");
        assert_eq!(chain.len(), 3);
    }

    #[test]
    fn chain_accepts_when_all_members_accept() {
        let chain = ChainCritic::new("happy-chain")
            .push(Box::new(NoopCritic))
            .push(Box::new(NoopCritic));
        let p = Proposal {
            kind: "test".into(),
            payload: serde_json::json!({}),
            context: Default::default(),
        };
        assert!(chain.evaluate(&p).is_accept());
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
