# Critic trait calling convention

**Status:** stable contract. Closes HANDOFF.md item #6. Downstream
forks (Forge-LFI, future application-specific Critics) build against
this contract.

## The trait

```rust
pub trait Critic: Send + Sync {
    fn evaluate(&self, proposal: &Proposal) -> Decision;
    fn ident(&self) -> &'static str;
}
```

Two methods. Three semantic invariants. One commit boundary.

## Semantic invariants

### 1. The commit boundary accepts `Decision`, never `Proposal`.

Every call site that wants to commit / ship / publish a piece of
generated content holds a `&dyn Critic` and calls `.evaluate()`
first. This is the seam — the compiler enforces that nothing
crosses without going through.

```rust
// Forbidden — payload reaches commit without Decision.
fn ship(payload: &str) { ... }            // BAD

// Required — only Decision crosses.
fn ship(decision: &Decision, payload: &str) {
    if !decision.is_accept() { return; }
    // ... commit payload here ...
}
```

Forge-side adapters (the `forge-critic` crate in PlausiDen-Forge)
hold a `&dyn Critic`, dispatch every CMS proposal through it, and
match on the returned Decision before writing to disk.

### 2. `evaluate` is referentially transparent within a single Critic
state.

Given the same `Proposal` + the same internal Critic state, two
calls to `evaluate` must produce the same `Decision`. Implementations
MAY observe internal state — corpora evolve, policy libraries hot-
swap — but two calls under identical state must agree.

This invariant is what makes Decisions replayable + auditable. If a
Critic returns different decisions for the same Proposal in the same
state, audit-chain replay diverges and the report loses signal.

### 3. `ident()` is a stable kebab-case slug.

Used in audit logs so a `Decision` traces back to which Critic
issued it. Stability matters: renaming a Critic's ident across a
release breaks the audit chain's identity continuity.

Reserved slugs:
* `noop` — `NoopCritic` (always-Accept).
* `lfi-reference` — `LfiCritic` (this crate's reference impl).
* `forge-default` — `ForgeCritic` in PlausiDen-Forge.

Application Critics pick distinct slugs in their own namespace.

## Decision payload contract

The three Decision variants encode three downstream actions:

| Variant   | What commit-side does                                      |
|-----------|-------------------------------------------------------------|
| `Accept`  | Commit the proposal. `confidence` may flow to the report.   |
| `Reject`  | Drop the proposal. `violations` flow to the operator UI.    |
| `Refine`  | Re-submit to the generator with `targeted_regeneration_guidance`. The generator MUST treat the guidance as a constraint (not a suggestion). |

`Refine` decisions are application-optional — a pipeline that
doesn't support regeneration may treat `Refine` as `Reject`. The
audit log distinguishes which happened.

## Variant-agnostic accessors

Per the helper methods added in commit 62768f1:

```rust
decision.is_accept() / is_reject() / is_refine();
decision.violations()      -> &[Violation]
decision.confidence()      -> Option<Strength>
decision.fired_rules()     -> &[(RuleId, Strength)]
```

These let consumers extract data without matching all three arms.

## Thread-safety

`Critic: Send + Sync` is part of the trait. Implementations must
support concurrent `evaluate` calls from multiple threads. Typically
this means:

* No mutable state in fields, OR
* Interior mutability via `Mutex` / `RwLock` / atomic operations.

Forge dispatches Critics from a tokio runtime (the runner pools
across many parallel page renders), so a non-Send Critic doesn't
fit the call site.

## State management

The reference `LfiCritic` stores `policy: PolicyLibrary` and
`corpus: Option<Corpus>` as immutable fields. Updates are
"swap the whole instance" — there is no `LfiCritic::update_policy`
method by design. This keeps the referential-transparency contract
honest: a given `&LfiCritic` binding evaluates the SAME way for its
lifetime; rotating policies is a higher-level orchestration
concern.

## Error handling

`evaluate` returns `Decision` infallibly. There is no
`Result<Decision, _>` shape — an evaluation that "couldn't decide"
is itself a Decision (typically `Reject` with a violation citing the
internal failure, or `Refine` with guidance to retry).

This shape was chosen so call sites can't accidentally `unwrap()` a
Critic failure into shipped content.

## Versioning the trait

The trait surface (`evaluate` + `ident`) is `0.1`. Future additions:

* `fn batch_evaluate(&self, proposals: &[Proposal]) -> Vec<Decision>` —
  optional default-impl that calls `evaluate` per item; specialized
  Critics may override for shared work (e.g. one HDC similarity
  scan across the batch).
* `fn explain(&self, proposal: &Proposal, decision: &Decision) -> String` —
  operator-facing explanation that goes beyond `Violation.explanation`.

Both are additive — the trait gains default-impl methods, downstream
Critics don't break.

Breaking changes (renames / removals of `evaluate` / `ident`)
require a major version bump on `lfi-critic` AND coordinated bumps
on every consumer.

## How Forge consumes

Today Forge has no Critic integration (the `forge-critic` crate is
queued — task #101 is the larger "complete-Rust-stack codegen"
work). When it lands, the integration shape will be:

```rust
// In forge-cli main or in the render phase:
let critic: Box<dyn Critic> = Box::new(LfiCritic {
    policy: forge_default(),
    corpus: load_tenant_corpus()?,
});

for proposal in proposals_from_cms() {
    let decision = critic.evaluate(&proposal);
    match decision {
        d if d.is_accept() => commit_to_static(&proposal),
        d if d.is_refine() => send_back_to_generator(&proposal, d.violations()),
        _ => log_reject(&proposal, d.violations()),
    }
}
```

The Critic trait stays a `Box<dyn Critic>` so Forge can swap impls
without recompiling (load NoopCritic for operator-authored content,
LfiCritic for LLM-generated content, application-specific Critics
for high-stakes flows).

## What is intentionally NOT specified

* The application-side mapping from `Atom.name` to evaluation
  probes. Each Critic that consumes a `PolicyLibrary` (like the
  forthcoming LfiCritic NeuPSL wiring) defines its own atom-
  evaluator. The trait calling convention doesn't constrain this.

* The format of `Proposal.payload` (it's `serde_json::Value`).
  Application forks define their typed schemas + rehydrate from
  the JSON inside their Critic impl.

* How a Critic interacts with the rest of the platform — that's
  the host's choice (sync dispatch vs queued background, etc.).

## Audit log shape (reference)

```jsonl
{"t": 0, "kind": "lfi.critic.evaluate", "critic_ident": "lfi-reference", "proposal_kind": "cms-section", "decision_kind": "accept", "confidence": 0.92, "fired_rule_ids": ["wcag-contrast-aa", "cms-section-required-fields"]}
```

Pinned shape so post-hoc replay walks the chain deterministically.
