# PlausiDen-LFI

**Lattice of Formal Inference** — generic, use-case-agnostic
neurosymbolic AI evaluation substrate for any application that
needs an auditable, interpretable AI decision layer.

## What it is

LFI is the **upstream evaluator** for AI proposals. Where most
AI-platform architectures put the LLM at the center and audit
results retroactively, LFI inverts the model:

- **LLM** is a constrained candidate generator (fluent copy,
  open-ended intent extraction).
- **LFI** is the decision-maker (policy rules, similarity
  geometry, brand consistency, abuse detection, drift).
- The **`Critic` trait** is the compiler-enforced seam: every
  proposal goes `Proposal → Critic::evaluate → Decision`. Raw
  LLM output cannot reach `commit()` — that's a type error,
  not a policy.

## Crates

| Crate         | Role                                              |
|---------------|---------------------------------------------------|
| `lfi-core`    | Typed transport: `Proposal`, `Decision`, `RuleId`, `Strength`, `Violation`. **Intentionally NSTK-free** — keeps transport swappable. |
| `lfi-policy`  | NeuPSL (Probabilistic Soft Logic) DSL — weighted rules, atoms, constraints. Will import `neupsl` from NSTK (#35). |
| `lfi-corpus`  | HDC-encoded curated patterns + per-tenant private corpora. Imports `hdc-core` from NSTK behind the `hdc-encoder` feature. |
| `lfi-critic`  | The `Critic` trait + `NoopCritic` + `RejectAllCritic` + `ChainCritic` + `LfiCritic` reference impl. |

### NSTK consumption is layered

Neurosymbolic-Toolkit (NSTK) substrates plug into the substrate-
consuming crates, NOT into `lfi-core`. This keeps `lfi-core`'s
transport types compatible with downstream forks that bring their
own (or no) substrate. Closes #34 by making the layering explicit:
the right place to depend on NSTK is `lfi-corpus` / `lfi-policy`,
not `lfi-core`.

## Upstream/downstream

This is the **generic upstream**. Application-specific forks
(e.g. [Forge-LFI](https://github.com/thepictishbeast/Forge-LFI))
maintain a separate repo that:

- treats PlausiDen-LFI as a git remote called `upstream`
- pulls upstream improvements via `git fetch upstream && git merge upstream/main`
- adds domain-specific Critic implementations, NeuPSL rules,
  HDC corpora on top

Don't put application-specific code in this repo. If it would
only make sense for Forge, or Shield, or Sacred.Vote, it
belongs downstream.

## Status

Scaffold. Typed surface defined; runtime impls land
incrementally. The Critic trait + `NoopCritic` work today; the
`LfiCritic` reference impl is a stub awaiting policy + corpus
content.

## License

MIT OR Apache-2.0.
