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
}

#[cfg(test)]
mod tests {
    use super::*;

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
