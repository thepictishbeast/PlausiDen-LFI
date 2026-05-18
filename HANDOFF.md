# Handoff — for the next LFI/AI Claude session

This repo was scaffolded 2026-05-18 by the Forge-side instance.
Paul has a separate instance focused on LFI/AI; everything below
is theirs to evolve.

## What's here

Four crates form the **typed application layer** over the
Neurosymbolic-Toolkit substrate:

```
crates/lfi-core    — Proposal, Decision, RuleId, Strength, Violation
crates/lfi-policy  — NeuPSL DSL: Rule, Atom, Literal, PolicyLibrary
crates/lfi-corpus  — HDC patterns + per-tenant private corpora
crates/lfi-critic  — Critic trait + NoopCritic + LfiCritic skeleton
```

`cargo build --workspace` + `cargo test --workspace` are green.
`#[forbid(unsafe_code)]` + `#[deny(missing_docs)]` enforced on
every crate.

## What's NOT here

- No NeuPSL solver wiring — `LfiCritic::evaluate` is a stub
  that returns `Accept` so call sites compile + test.
- No HDC encoder/decoder — `Pattern::vector_b64` is opaque.
- No real policies in `lfi-policy::PolicyLibrary` — just the
  type for building libraries.
- No curated corpora in `lfi-corpus::Corpus`.

## What the LFI instance should know

1. **Architecture context** is in
   `/home/paul/projects/PlausiDen-Forge/docs/PLATFORM_ROADMAP.md`
   §6 — the LFI-first / LLM-peripheral inversion + Critic trait
   as compiler-enforced seam.

2. **Substrate** is in
   [`Neurosymbolic-Toolkit`](https://github.com/thepictishbeast/Neurosymbolic-Toolkit)
   — already public, has hdc-core / neupsl / lnn / vsa / hdlm /
   math-codec. Paul flagged on 2026-05-18 that we might
   consolidate (rename NSTK → PlausiDen-LFI + merge this
   scaffold's crates into the same workspace). See
   [`UPSTREAM.md`](UPSTREAM.md) for the layering note.

3. **Downstream fork** target: `Forge-LFI` — does not exist
   yet. Forge-side Critic impls, NeuPSL rule libraries
   (WCAG-A11y, brand-voice, typed-CMS invariants), HDC
   curated corpora. Treats this repo as `upstream` git
   remote; merges flow downstream.

4. **Memory** in `/root/.claude/projects/-/memory/` carries:
   - `feedback_lfi_as_core_llm_as_peripheral.md` (architecture)
   - `feedback_manifest_layer_is_the_keystone.md` (priority)
   - `feedback_super_society_tech_stack.md` (axes the Critic
     should ultimately enforce: fast + reliable + robust +
     secure + anonymous + private SIMULTANEOUSLY)

5. **Forge-side seam**: the Forge-LFI fork will impl `Critic`
   for a `ForgeCritic` type and Forge will dispatch through
   `&dyn Critic`. The PlausiDen-LFI Critic trait + types are
   the contract; Forge-side adapters consume them.

## Suggested next steps

1. Decide consolidation: rename NSTK → PlausiDen-LFI + merge
   this scaffold's crates? Or keep as two layers? See UPSTREAM.md.
2. If keeping separate, push this repo to
   `github.com/thepictishbeast/PlausiDen-LFI` (or paul's chosen
   name).
3. Wire `lfi-corpus::Pattern::vector_b64` to the actual
   `hdc-core::BipolarVector` encoder/decoder.
4. Replace `LfiCritic::evaluate` stub with a real NeuPSL solve
   over `self.policy` + HDC similarity pass over `self.corpus`.
5. Author the first generic NeuPSL policy library (e.g.
   `library://default/originality-similarity`).
6. Document the trait calling convention so the Forge-side
   instance can build `Forge-LFI` against a stable contract.

## What this instance (Forge-side) will NOT touch

- Any of the above. Per paul's 2026-05-18 directive: this
  repo is the LFI instance's. Forge-side PRs against this
  repo should only be from there.
- The trait surface in `lfi-critic::Critic` is the public
  contract for Forge-LFI; if a breaking change is needed,
  coordinate with the Forge instance via paul.

## Build sanity

```bash
cd /home/paul/projects/PlausiDen-LFI
cargo build --workspace
cargo test --workspace
```

Both clean as of initial scaffold (c95e5ac).
