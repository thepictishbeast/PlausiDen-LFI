//! Canonical policy libraries — HANDOFF item #5.
//!
//! Closes the "Author the first generic NeuPSL policy library"
//! item from HANDOFF.md. Provides one canonical library that
//! downstream Critic implementations can use as a baseline:
//!
//! * `forge_default()` — the rule set Forge-LFI's Critic
//!   evaluates by default. Covers WCAG-A11y baseline, brand-
//!   voice, typed-CMS invariants, originality-similarity floor.
//!
//! Authoring more libraries (per-tenant, per-vertical) follows
//! the same shape — write a function returning a `PolicyLibrary`
//! with a stable version slug.
//!
//! ## Rule families
//!
//! * **Hard** (immediate-reject on violation):
//!   - `wcag-contrast-aa` — body must pass-contrast-aa.
//!   - `cms-section-required-fields` — body must
//!     pass-required-field-check.
//!   - `no-third-party-trackers` — body must have no
//!     external-tracker references.
//!   - `valid-canonical-url` — page must have a canonical URL.
//! * **Soft** (weighted penalty):
//!   - `brand-voice-passive-cap` (weight 0.7) — passive-voice
//!     density should be under tenant cap.
//!   - `originality-similarity-cap` (weight 0.8) — corpus-
//!     similarity should be under tenant cap (anti-sameness).
//!   - `density-tier-floor` (weight 0.6) — content density
//!     should meet the declared tier's floor.
//!   - `editorial-paragraph-cap` (weight 0.5) — paragraph
//!     count above scaffold-only threshold.
//!   - `slop-dictionary-cap` (weight 0.9) — slop-pattern matches
//!     should be zero (per
//!     `PlausiDen-Forge/docs/REFERENCE_CORPUS.md`).
//!
//! Each rule's atoms (`name` + `args`) are application-specific;
//! the Critic that consumes the library maps atoms to evaluation
//! probes (parser checks, corpus lookups, layout audits).

use lfi_core::{RuleId, Strength};

use crate::{Atom, Hardness, Literal, Polarity, PolicyLibrary, Rule};

/// The default Forge-LFI policy library. Version-pinned;
/// downstream consumers may pin or hot-swap.
#[must_use]
pub fn forge_default() -> PolicyLibrary {
    let mut lib = PolicyLibrary::new("forge-default", "0.1.0");
    lib.rules.extend(hard_rules());
    lib.rules.extend(soft_rules());
    lib
}

fn hard_rules() -> Vec<Rule> {
    vec![
        Rule {
            id: RuleId::new("wcag-contrast-aa"),
            description: "Every text-foreground pair must meet WCAG 2.1 AA contrast (4.5:1 normal text, 3:1 large text).".to_owned(),
            hardness: Hardness::Hard,
            weight: Strength::FULL,
            body: vec![positive("passes-contrast-aa", &[])],
            head: atom("acceptable", &[]),
        },
        Rule {
            id: RuleId::new("cms-section-required-fields"),
            description: "Every CmsSection variant has its required typed fields filled.".to_owned(),
            hardness: Hardness::Hard,
            weight: Strength::FULL,
            body: vec![positive("cms-section-has-required-fields", &[])],
            head: atom("acceptable", &[]),
        },
        Rule {
            id: RuleId::new("no-third-party-trackers"),
            description: "Page contains no cross-origin script / pixel / iframe trackers.".to_owned(),
            hardness: Hardness::Hard,
            weight: Strength::FULL,
            body: vec![negative("has-third-party-tracker", &[])],
            head: atom("acceptable", &[]),
        },
        Rule {
            id: RuleId::new("valid-canonical-url"),
            description: "Page declares a same-origin <link rel=\"canonical\"> with a valid URL.".to_owned(),
            hardness: Hardness::Hard,
            weight: Strength::FULL,
            body: vec![positive("has-valid-canonical", &[])],
            head: atom("acceptable", &[]),
        },
    ]
}

fn soft_rules() -> Vec<Rule> {
    vec![
        Rule {
            id: RuleId::new("brand-voice-passive-cap"),
            description: "Passive-voice density should be under the tenant's declared cap (default 0.15).".to_owned(),
            hardness: Hardness::Soft,
            weight: Strength::new(0.7),
            body: vec![positive("passive-voice-density-under-cap", &[])],
            head: atom("brand-voice-on-tone", &[]),
        },
        Rule {
            id: RuleId::new("originality-similarity-cap"),
            description: "Page's HDC similarity to the tenant's prior pages should be under the cap (anti-sameness).".to_owned(),
            hardness: Hardness::Soft,
            weight: Strength::new(0.8),
            body: vec![positive("corpus-similarity-under-cap", &["tenant-private"])],
            head: atom("distinct-from-tenant-history", &[]),
        },
        Rule {
            id: RuleId::new("density-tier-floor"),
            description: "Content density meets the floor for the declared density tier (press / editorial / commerce / minimal).".to_owned(),
            hardness: Hardness::Soft,
            weight: Strength::new(0.6),
            body: vec![positive("density-meets-tier-floor", &[])],
            head: atom("density-sufficient", &[]),
        },
        Rule {
            id: RuleId::new("editorial-paragraph-cap"),
            description: "Page has paragraphs / headings / kv_pairs above the scaffold-only threshold (≥ 1).".to_owned(),
            hardness: Hardness::Soft,
            weight: Strength::new(0.5),
            body: vec![positive("has-editorial-body", &[])],
            head: atom("editorial-content-present", &[]),
        },
        Rule {
            id: RuleId::new("slop-dictionary-cap"),
            description: "Page matches zero slop-dictionary anti-patterns per docs/REFERENCE_CORPUS.md.".to_owned(),
            hardness: Hardness::Soft,
            weight: Strength::new(0.9),
            body: vec![negative("matches-slop-dictionary", &[])],
            head: atom("distinct-from-slop-corpus", &[]),
        },
    ]
}

fn atom(name: &str, args: &[&str]) -> Atom {
    Atom {
        name: name.to_owned(),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
    }
}

fn positive(name: &str, args: &[&str]) -> Literal {
    Literal {
        atom: atom(name, args),
        polarity: Polarity::Positive,
    }
}

fn negative(name: &str, args: &[&str]) -> Literal {
    Literal {
        atom: atom(name, args),
        polarity: Polarity::Negative,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_default_library_has_expected_shape() {
        let lib = forge_default();
        assert_eq!(lib.name, "forge-default");
        assert_eq!(lib.version, "0.1.0");
        assert_eq!(lib.hard_count(), 4);
        assert_eq!(lib.soft_count(), 5);
        assert_eq!(lib.rules.len(), 9);
    }

    #[test]
    fn forge_default_rule_ids_are_unique() {
        let lib = forge_default();
        let mut seen = std::collections::HashSet::new();
        for r in &lib.rules {
            assert!(
                seen.insert(r.id.clone()),
                "duplicate rule id: {}",
                r.id
            );
        }
    }

    #[test]
    fn forge_default_serde_roundtrip() {
        let lib = forge_default();
        let j = serde_json::to_string(&lib).expect("ser");
        let back: PolicyLibrary = serde_json::from_str(&j).expect("de");
        assert_eq!(back.rules.len(), 9);
        assert_eq!(back.name, "forge-default");
    }

    #[test]
    fn hard_rules_have_full_strength() {
        let lib = forge_default();
        for r in lib.rules.iter().filter(|r| r.hardness == Hardness::Hard) {
            assert_eq!(r.weight.get(), 1.0, "hard rule {} has weight != 1.0", r.id);
        }
    }

    #[test]
    fn soft_rule_weights_are_below_1() {
        let lib = forge_default();
        for r in lib.rules.iter().filter(|r| r.hardness == Hardness::Soft) {
            assert!(
                r.weight.get() < 1.0,
                "soft rule {} has weight {} ≥ 1.0",
                r.id,
                r.weight.get()
            );
        }
    }

    #[test]
    fn soft_rule_weights_reflect_priority_ordering() {
        // Sanity: slop-dictionary should outweigh editorial-cap,
        // originality should outweigh brand-voice. Pins doctrine
        // weights — if these flip, someone changed priority and
        // should justify it in the commit.
        let lib = forge_default();
        let weight_of = |id: &str| -> f32 {
            lib.rules
                .iter()
                .find(|r| r.id.as_str() == id)
                .map(|r| r.weight.get())
                .unwrap_or(0.0)
        };
        assert!(weight_of("slop-dictionary-cap") > weight_of("editorial-paragraph-cap"));
        assert!(weight_of("originality-similarity-cap") > weight_of("brand-voice-passive-cap"));
    }
}
