# Upstream / downstream layering

This repo (**PlausiDen-LFI**) is the **typed application layer**
over the substrate. It exposes `Proposal`, `Decision`, `Critic`,
`PolicyLibrary`, `Corpus` — the boundary every application
(Forge, Shield, Sacred.Vote, Engine, etc.) speaks to.

The actual neurosymbolic math (HDC vectors, NeuPSL solver,
LNN, VSA, HDLM, math-codec) lives in
[**Neurosymbolic-Toolkit**](https://github.com/thepictishbeast/Neurosymbolic-Toolkit)
as the substrate. This repo consumes it as a Cargo git
dependency.

## Possible consolidation (paul-side decision)

Paul flagged on 2026-05-18 that we might want one combined
repo — rename `Neurosymbolic-Toolkit` to `PlausiDen-LFI` and
merge the typed application layer into the same workspace.
The renamed workspace would then carry:

- `hdc-core`, `neupsl`, `lnn`, `vsa`, `hdlm`, `math-codec`,
  `neurosymbolic` (existing substrate)
- `lfi-core`, `lfi-policy`, `lfi-corpus`, `lfi-critic`
  (typed application layer added by this scaffold)

If we go that route:

1. Rename the GitHub repo: `Neurosymbolic-Toolkit` → `PlausiDen-LFI`
2. Add this scaffold's four crates into the renamed workspace
3. Delete this standalone repo (or convert it into a redirect)
4. Add a `HISTORY.md` noting "this used to be called
   Neurosymbolic-Toolkit"

The alternative is to keep them separate (substrate vs.
application API), which is the current shape.

## Downstream

[**Forge-LFI**](https://github.com/thepictishbeast/Forge-LFI)
is the downstream fork tailored for Forge — adds Forge-specific
`Critic` impls, NeuPSL rule libraries (WCAG-A11y, brand-voice,
typed-CMS invariants), and HDC corpora (curated Loom primitives
for similarity / originality checks).

Forge-LFI treats this repo as a git remote named `upstream`.
Upstream improvements trickle downstream via:

```bash
cd Forge-LFI
git fetch upstream
git merge upstream/main
# resolve conflicts (Forge-specific adapters that touched
# the same files as upstream changes)
cargo build && cargo test
```
