//! `lfi-corpus` — HDC-encoded curated patterns + per-tenant
//! private corpora.
//!
//! A **corpus** is a typed collection of `Pattern`s. Each
//! pattern carries:
//!
//! - HDC vector (10k-dimensional bipolar; from the
//!   Neurosymbolic-Toolkit's `hdc-core`)
//! - kebab-case slug
//! - optional metadata (origin, tags, retention class)
//!
//! Critics use corpora for:
//!
//! - **originality**: cosine-similarity threshold against
//!   the curated patterns ("is this 92% similar to an
//!   existing site? reject")
//! - **brand consistency**: similarity against a tenant's
//!   private brand corpus ("does this copy align with
//!   prior approved tone?")
//! - **abuse detection**: clustering against known-bad
//!   patterns
//!
//! ## Per-tenant isolation
//!
//! Corpora are tagged with a `tenant_id` (or `None` for
//! curated/public). Critic implementations MUST respect
//! `tenant_id` — tenant A's corpus cannot influence
//! decisions for tenant B.
//!
//! ## Status
//!
//! Typed surface scaffold. Actual HDC encoding/lookup is
//! deferred to the implementation phase; this crate ships
//! the types so policy and critic crates can be authored
//! against a stable shape.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(feature = "hdc-encoder")]
pub mod hdc_encoder;

use serde::{Deserialize, Serialize};

/// HDC vector dimension. The Neurosymbolic-Toolkit's hdc-core
/// uses 10,000-d bipolar by default. Kept generic here so a
/// downstream corpus can use a different dimension if needed.
pub const DEFAULT_HDC_DIM: usize = 10_000;

/// A single curated pattern.
///
/// `vector` is base64-encoded packed-bipolar bytes for
/// transport efficiency. The downstream encoder/decoder
/// lives in the application Critic adapter (downstream forks
/// like Forge-LFI handle the Neurosymbolic-Toolkit conversion).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Pattern {
    /// Stable kebab-case slug.
    pub slug: String,
    /// Human-readable description.
    pub description: String,
    /// Vector dimensions (typically `DEFAULT_HDC_DIM`).
    pub dim: usize,
    /// Base64-encoded packed bipolar vector.
    pub vector_b64: String,
    /// Free-form tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Provenance — where this pattern came from.
    pub origin: PatternOrigin,
}

/// Where a pattern came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternOrigin {
    /// Hand-curated by a policy team.
    Curated,
    /// Imported from an existing approved artifact.
    Imported,
    /// Synthesized from an LLM proposal that was accepted by
    /// a Critic (closing the loop: good outputs become future
    /// reference patterns).
    Synthesized,
}

/// Retention class for corpus entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RetentionClass {
    /// Curated reference. Indefinite retention.
    Reference,
    /// Tenant-private. Retained per tenant's data policy.
    TenantPrivate,
    /// Ephemeral / cache. Discarded on rotation.
    Ephemeral,
}

/// One corpus — a named collection of patterns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Corpus {
    /// Corpus name (e.g. `forge-default-ui`, `tenant-acme-brand`).
    pub name: String,
    /// Corpus version (bump on content change).
    pub version: String,
    /// Tenant scope (None = curated/public).
    pub tenant_id: Option<String>,
    /// Retention class.
    pub retention: RetentionClass,
    /// The patterns.
    pub patterns: Vec<Pattern>,
}

impl Corpus {
    /// Construct an empty curated corpus.
    pub fn curated(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            tenant_id: None,
            retention: RetentionClass::Reference,
            patterns: Vec::new(),
        }
    }

    /// Construct an empty tenant-private corpus.
    pub fn tenant_private(
        name: impl Into<String>,
        version: impl Into<String>,
        tenant: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            tenant_id: Some(tenant.into()),
            retention: RetentionClass::TenantPrivate,
            patterns: Vec::new(),
        }
    }

    /// Look up a pattern by slug. Returns `None` if no pattern
    /// with that slug exists in this corpus.
    pub fn get(&self, slug: &str) -> Option<&Pattern> {
        self.patterns.iter().find(|p| p.slug == slug)
    }

    /// True iff a pattern with the given slug exists.
    pub fn contains(&self, slug: &str) -> bool {
        self.get(slug).is_some()
    }

    /// Number of patterns in the corpus.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// True iff the corpus has zero patterns.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Find the top-K patterns most similar to `probe` by cosine
    /// similarity over the HDC-decoded vector. Returns
    /// `(&Pattern, similarity)` pairs sorted descending by
    /// similarity.
    ///
    /// Patterns whose `vector_b64` fails to decode are skipped
    /// silently (callers can pre-validate with `Corpus::validate`
    /// if strict behaviour is needed).
    ///
    /// Behind the `hdc-encoder` feature — same constraint as the
    /// codec it builds on.
    #[cfg(feature = "hdc-encoder")]
    pub fn find_similar(
        &self,
        probe: &hdc_core::BipolarVector,
        top_k: usize,
    ) -> Vec<(&Pattern, f64)> {
        if top_k == 0 || self.patterns.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(&Pattern, f64)> = self
            .patterns
            .iter()
            .filter_map(|p| {
                let v = crate::hdc_encoder::pattern_to_vector(p).ok()?;
                let sim = probe.cosine_similarity(&v).ok()?;
                Some((p, sim))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(top_k);
        scored
    }

    /// Like [`find_similar`] but returns only patterns scoring at
    /// or above `min_similarity`. Combine with `top_k` to bound the
    /// result list when you also want a quality floor.
    ///
    /// Returned list is sorted descending by similarity.
    ///
    /// Behind the `hdc-encoder` feature.
    #[cfg(feature = "hdc-encoder")]
    pub fn find_similar_above(
        &self,
        probe: &hdc_core::BipolarVector,
        min_similarity: f64,
        top_k: usize,
    ) -> Vec<(&Pattern, f64)> {
        self.find_similar(probe, top_k)
            .into_iter()
            .filter(|(_, s)| *s >= min_similarity)
            .collect()
    }

    /// Validate corpus invariants. Returns the list of every
    /// violation found — empty Vec means the corpus is sound.
    ///
    /// Checks (in stable order):
    ///
    /// 1. Corpus `name` is non-empty.
    /// 2. Corpus `version` is non-empty.
    /// 3. No two patterns share a `slug` (slug is the
    ///    similarity-lookup primary key — duplicates make
    ///    cosine queries non-deterministic).
    /// 4. Every pattern's `slug` is non-empty.
    /// 5. Every pattern's `dim` matches the corpus's first
    ///    pattern's dim (mixed-dim corpora can't be queried
    ///    against a single probe vector).
    /// 6. Tenant isolation: tenant-private retention requires
    ///    `Some(tenant_id)`; reference retention requires
    ///    `None`.
    pub fn validate(&self) -> Vec<CorpusValidationError> {
        let mut errors = Vec::new();
        if self.name.is_empty() {
            errors.push(CorpusValidationError::EmptyCorpusName);
        }
        if self.version.is_empty() {
            errors.push(CorpusValidationError::EmptyCorpusVersion);
        }
        let mut seen_slugs: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let first_dim = self.patterns.first().map(|p| p.dim);
        for pattern in &self.patterns {
            if pattern.slug.is_empty() {
                errors.push(CorpusValidationError::EmptyPatternSlug);
            } else if !seen_slugs.insert(pattern.slug.clone()) {
                errors.push(CorpusValidationError::DuplicatePatternSlug(
                    pattern.slug.clone(),
                ));
            }
            if let Some(d0) = first_dim {
                if pattern.dim != d0 {
                    errors.push(CorpusValidationError::MixedDimension {
                        slug: pattern.slug.clone(),
                        expected: d0,
                        got: pattern.dim,
                    });
                }
            }
        }
        match (self.retention, self.tenant_id.as_deref()) {
            (RetentionClass::TenantPrivate, None) => {
                errors.push(CorpusValidationError::TenantPrivateMissingTenant);
            }
            (RetentionClass::Reference, Some(_)) => {
                errors.push(CorpusValidationError::ReferenceCorpusWithTenant);
            }
            _ => {}
        }
        errors
    }

    /// True iff `validate()` returns no errors.
    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

/// Validation errors returned by [`Corpus::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusValidationError {
    /// `corpus.name` is empty.
    EmptyCorpusName,
    /// `corpus.version` is empty.
    EmptyCorpusVersion,
    /// Two patterns share this slug.
    DuplicatePatternSlug(String),
    /// A pattern has an empty slug.
    EmptyPatternSlug,
    /// A pattern's `dim` differs from the corpus's first
    /// pattern's `dim` — corpus is mixed-dimension.
    MixedDimension {
        /// Offending pattern's slug.
        slug: String,
        /// Dim expected (from first pattern).
        expected: usize,
        /// Dim found on this pattern.
        got: usize,
    },
    /// Retention is `TenantPrivate` but `tenant_id` is `None`.
    TenantPrivateMissingTenant,
    /// Retention is `Reference` but `tenant_id` is `Some` —
    /// a curated corpus must not be scoped to a tenant.
    ReferenceCorpusWithTenant,
}

impl std::fmt::Display for CorpusValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCorpusName => write!(f, "corpus.name is empty"),
            Self::EmptyCorpusVersion => write!(f, "corpus.version is empty"),
            Self::DuplicatePatternSlug(s) => write!(f, "duplicate pattern slug: {s}"),
            Self::EmptyPatternSlug => write!(f, "pattern has empty slug"),
            Self::MixedDimension { slug, expected, got } => write!(
                f,
                "pattern {slug} has dim {got}, expected {expected} (mixed-dim corpus)"
            ),
            Self::TenantPrivateMissingTenant => {
                write!(f, "tenant-private corpus has no tenant_id")
            }
            Self::ReferenceCorpusWithTenant => {
                write!(f, "reference corpus has a tenant_id (should be None)")
            }
        }
    }
}

impl std::error::Error for CorpusValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "hdc-encoder")]
    mod find_similar_tests {
        use super::super::*;
        use crate::hdc_encoder;
        use hdc_core::{BipolarVector, HdcParams};

        fn make_corpus_with_seeds(seeds: &[(u64, &str)]) -> Corpus {
            let mut c = Corpus::curated("similar-test", "0.1");
            let params = HdcParams {
                dimensions: 10_000,
                ..Default::default()
            };
            for (seed, slug) in seeds {
                let v = BipolarVector::random_seeded(&params, *seed);
                c.patterns.push(hdc_encoder::pattern_from_vector(
                    &v,
                    *slug,
                    "test pattern",
                    vec![],
                    PatternOrigin::Curated,
                ));
            }
            c
        }

        #[test]
        fn empty_corpus_returns_empty() {
            let c = Corpus::curated("empty", "0.1");
            let params = HdcParams::default();
            let probe = BipolarVector::random_seeded(&params, 1);
            assert_eq!(c.find_similar(&probe, 5).len(), 0);
        }

        #[test]
        fn top_k_zero_returns_empty() {
            let c = make_corpus_with_seeds(&[(1, "a"), (2, "b")]);
            let params = HdcParams::default();
            let probe = BipolarVector::random_seeded(&params, 99);
            assert_eq!(c.find_similar(&probe, 0).len(), 0);
        }

        #[test]
        fn exact_match_returns_self_first() {
            let c = make_corpus_with_seeds(&[(1, "alpha"), (2, "beta"), (3, "gamma")]);
            // Probe = exactly alpha's vector
            let alpha_vec = hdc_encoder::pattern_to_vector(&c.patterns[0]).unwrap();
            let results = c.find_similar(&alpha_vec, 3);
            assert_eq!(results.len(), 3);
            assert_eq!(results[0].0.slug, "alpha", "exact match should rank first");
            // Self-similarity should be 1.0 (or very close after roundtrip).
            assert!(
                results[0].1 > 0.999,
                "self-similarity should be ~1.0, got {}",
                results[0].1
            );
            // Other two should be near orthogonal (random unrelated
            // bipolar vectors at D=10k have cosine within ±0.02).
            assert!(results[1].1.abs() < 0.05);
            assert!(results[2].1.abs() < 0.05);
        }

        #[test]
        fn results_sorted_descending() {
            let c = make_corpus_with_seeds(&[(1, "a"), (2, "b"), (3, "c"), (4, "d"), (5, "e")]);
            let params = HdcParams::default();
            let probe = BipolarVector::random_seeded(&params, 1);
            let results = c.find_similar(&probe, 5);
            assert_eq!(results.len(), 5);
            for w in results.windows(2) {
                assert!(
                    w[0].1 >= w[1].1,
                    "results out of order: {} < {}",
                    w[0].1,
                    w[1].1
                );
            }
        }

        #[test]
        fn top_k_caps_result_size() {
            let c = make_corpus_with_seeds(&[(1, "a"), (2, "b"), (3, "c"), (4, "d")]);
            let params = HdcParams::default();
            let probe = BipolarVector::random_seeded(&params, 99);
            assert_eq!(c.find_similar(&probe, 2).len(), 2);
        }

        #[test]
        fn find_similar_above_filters_by_threshold() {
            let c = make_corpus_with_seeds(&[(1, "alpha"), (2, "beta")]);
            let alpha_vec = hdc_encoder::pattern_to_vector(&c.patterns[0]).unwrap();

            // Tight threshold — only the exact match passes.
            let strict = c.find_similar_above(&alpha_vec, 0.95, 10);
            assert_eq!(strict.len(), 1);
            assert_eq!(strict[0].0.slug, "alpha");

            // Slack threshold — both pass (since random bipolar
            // pairs at D=10k tend to be near-orthogonal, the
            // beta entry will have |cos| << 0.95 but >= -1.0).
            // Set threshold = -1.0 to admit everything.
            let loose = c.find_similar_above(&alpha_vec, -1.0, 10);
            assert_eq!(loose.len(), 2);
        }

        #[test]
        fn skips_patterns_with_bad_b64() {
            // Build a corpus with one valid + one corrupt entry.
            let mut c = make_corpus_with_seeds(&[(1, "good")]);
            c.patterns.push(Pattern {
                slug: "broken".into(),
                description: "corrupt b64".into(),
                dim: 10_000,
                vector_b64: "###not-base64###".into(),
                tags: vec![],
                origin: PatternOrigin::Curated,
            });
            let params = HdcParams::default();
            let probe = BipolarVector::random_seeded(&params, 99);
            let results = c.find_similar(&probe, 5);
            // The broken pattern is silently skipped — only "good"
            // appears in results.
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].0.slug, "good");
        }
    }

    #[test]
    fn curated_constructor() {
        let c = Corpus::curated("test", "0.1");
        assert_eq!(c.tenant_id, None);
        assert_eq!(c.retention, RetentionClass::Reference);
        assert_eq!(c.patterns.len(), 0);
    }

    #[test]
    fn tenant_private_constructor() {
        let c = Corpus::tenant_private("brand", "0.1", "acme");
        assert_eq!(c.tenant_id.as_deref(), Some("acme"));
        assert_eq!(c.retention, RetentionClass::TenantPrivate);
    }

    #[test]
    fn empty_corpus_passes_validation() {
        assert!(Corpus::curated("test", "0.1").is_valid());
        assert!(Corpus::tenant_private("brand", "0.1", "acme").is_valid());
    }

    #[test]
    fn corpus_missing_name_fails() {
        let c = Corpus::curated("", "0.1");
        assert!(c
            .validate()
            .iter()
            .any(|e| matches!(e, CorpusValidationError::EmptyCorpusName)));
    }

    #[test]
    fn corpus_missing_version_fails() {
        let c = Corpus::curated("test", "");
        assert!(c
            .validate()
            .iter()
            .any(|e| matches!(e, CorpusValidationError::EmptyCorpusVersion)));
    }

    fn make_pattern(slug: &str, dim: usize) -> Pattern {
        Pattern {
            slug: slug.into(),
            description: "p".into(),
            dim,
            vector_b64: "AAAA".into(),
            tags: vec![],
            origin: PatternOrigin::Curated,
        }
    }

    #[test]
    fn duplicate_pattern_slug_fails_validation() {
        let mut c = Corpus::curated("test", "0.1");
        c.patterns
            .push(make_pattern("dup", DEFAULT_HDC_DIM));
        c.patterns
            .push(make_pattern("dup", DEFAULT_HDC_DIM));
        assert!(c
            .validate()
            .iter()
            .any(|e| matches!(e, CorpusValidationError::DuplicatePatternSlug(s) if s == "dup")));
    }

    #[test]
    fn empty_pattern_slug_fails_validation() {
        let mut c = Corpus::curated("test", "0.1");
        c.patterns.push(make_pattern("", DEFAULT_HDC_DIM));
        assert!(c
            .validate()
            .iter()
            .any(|e| matches!(e, CorpusValidationError::EmptyPatternSlug)));
    }

    #[test]
    fn mixed_dim_corpus_fails_validation() {
        let mut c = Corpus::curated("test", "0.1");
        c.patterns
            .push(make_pattern("a", DEFAULT_HDC_DIM));
        c.patterns.push(make_pattern("b", 5_000));
        let errs = c.validate();
        assert!(errs.iter().any(|e| matches!(
            e,
            CorpusValidationError::MixedDimension { slug, .. } if slug == "b"
        )));
    }

    #[test]
    fn tenant_private_missing_tenant_id_fails() {
        let mut c = Corpus::curated("test", "0.1");
        c.retention = RetentionClass::TenantPrivate;
        assert!(c
            .validate()
            .iter()
            .any(|e| matches!(e, CorpusValidationError::TenantPrivateMissingTenant)));
    }

    #[test]
    fn reference_corpus_with_tenant_fails() {
        let mut c = Corpus::tenant_private("brand", "0.1", "acme");
        c.retention = RetentionClass::Reference;
        // Still has tenant_id from tenant_private constructor.
        assert!(c
            .validate()
            .iter()
            .any(|e| matches!(e, CorpusValidationError::ReferenceCorpusWithTenant)));
    }

    #[test]
    fn get_returns_pattern_by_slug() {
        let mut c = Corpus::curated("test", "0.1");
        c.patterns.push(make_pattern("alpha", DEFAULT_HDC_DIM));
        c.patterns.push(make_pattern("beta", DEFAULT_HDC_DIM));
        assert_eq!(c.get("beta").map(|p| p.slug.as_str()), Some("beta"));
        assert!(c.get("missing").is_none());
        assert!(c.contains("alpha"));
        assert!(!c.contains("missing"));
        assert_eq!(c.len(), 2);
        assert!(!c.is_empty());
    }

    #[test]
    fn pattern_serde_roundtrip() {
        let p = Pattern {
            slug: "test".into(),
            description: "test".into(),
            dim: DEFAULT_HDC_DIM,
            vector_b64: "AAAA".into(),
            tags: vec![],
            origin: PatternOrigin::Curated,
        };
        let j = serde_json::to_string(&p).unwrap();
        let back: Pattern = serde_json::from_str(&j).unwrap();
        assert_eq!(p, back);
    }
}
