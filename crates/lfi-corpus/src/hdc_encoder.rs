//! Canonical encoder/decoder between [`Pattern::vector_b64`] and
//! [`hdc_core::BipolarVector`].
//!
//! NSTK's `BipolarVector` stores f32 per dimension (allows
//! intermediate bundle results between -1 and +1). On the wire we
//! pack to one bit per dimension (sign), then base64-encode the
//! resulting byte slice.
//!
//! Encoding loses the magnitude of any partially-bundled vector
//! — for transport we treat each dimension as `vec[i] > 0.0 ?
//! +1 : -1`. Round-tripping a vector that was already bipolar
//! (every element ±1.0) is exact.
//!
//! HANDOFF item #3 from the initial scaffold.
//!
//! Behind feature `hdc-encoder`. Off by default so downstream
//! forks that bring their own substrate don't pay the NSTK
//! compile cost.

use crate::Pattern;
use base64::Engine;
use hdc_core::BipolarVector;
use thiserror::Error;

/// Errors from HDC-encoder roundtrips.
#[derive(Debug, Error)]
pub enum HdcEncoderError {
    /// Pattern's declared dim doesn't match what was decoded.
    #[error("pattern dim mismatch: declared={declared}, decoded={decoded}")]
    DimMismatch {
        /// What the Pattern said.
        declared: usize,
        /// What actually decoded.
        decoded: usize,
    },
    /// The base64 string was malformed.
    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),
}

/// Pack a [`BipolarVector`] into the wire format used by
/// [`Pattern::vector_b64`]: one bit per dimension (1 if value
/// > 0.0 else 0), packed LSB-first into bytes, then base64.
pub fn encode(vector: &BipolarVector) -> String {
    let bits = vector.as_slice();
    let n_bytes = bits.len().div_ceil(8);
    let mut bytes = vec![0u8; n_bytes];
    for (i, &v) in bits.iter().enumerate() {
        if v > 0.0 {
            bytes[i / 8] |= 1u8 << (i % 8);
        }
    }
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

/// Decode a base64-packed bipolar string into a
/// [`BipolarVector`] of exactly `expected_dim` dimensions.
/// Bit `0` → `-1.0`, bit `1` → `+1.0`. Any trailing bits in the
/// final byte beyond `expected_dim` are ignored.
pub fn decode(b64: &str, expected_dim: usize) -> Result<BipolarVector, HdcEncoderError> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
    let available_bits = bytes.len() * 8;
    if available_bits < expected_dim {
        return Err(HdcEncoderError::DimMismatch {
            declared: expected_dim,
            decoded: available_bits,
        });
    }
    let mut data = Vec::with_capacity(expected_dim);
    for i in 0..expected_dim {
        let bit_set = (bytes[i / 8] >> (i % 8)) & 1 == 1;
        data.push(if bit_set { 1.0 } else { -1.0 });
    }
    Ok(BipolarVector::from_data(data))
}

/// Construct a [`Pattern`] from a [`BipolarVector`]. The
/// caller supplies slug, description, tags, origin; this
/// helper handles the encoding + dim consistency.
pub fn pattern_from_vector(
    vector: &BipolarVector,
    slug: impl Into<String>,
    description: impl Into<String>,
    tags: Vec<String>,
    origin: crate::PatternOrigin,
) -> Pattern {
    Pattern {
        slug: slug.into(),
        description: description.into(),
        dim: vector.dim(),
        vector_b64: encode(vector),
        tags,
        origin,
    }
}

/// Decode a [`Pattern`]'s `vector_b64` back into a
/// [`BipolarVector`], validating that the decoded dimension
/// matches the pattern's declared `dim`.
pub fn pattern_to_vector(pattern: &Pattern) -> Result<BipolarVector, HdcEncoderError> {
    decode(&pattern.vector_b64, pattern.dim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PatternOrigin;
    use hdc_core::HdcParams;

    fn make_bipolar(seed: u64, dim: usize) -> BipolarVector {
        let params = HdcParams {
            dimensions: dim,
            ..Default::default()
        };
        BipolarVector::random_seeded(&params, seed)
    }

    #[test]
    fn roundtrip_is_bit_exact_for_bipolar_input() {
        let v = make_bipolar(0xABCDEF, 10_000);
        let b64 = encode(&v);
        let back = decode(&b64, 10_000).unwrap();
        assert_eq!(v.as_slice(), back.as_slice());
    }

    #[test]
    fn small_dim_roundtrip() {
        let v = make_bipolar(0x42, 64);
        let b64 = encode(&v);
        let back = decode(&b64, 64).unwrap();
        assert_eq!(v.as_slice(), back.as_slice());
    }

    #[test]
    fn unaligned_dim_roundtrip() {
        // 17 bits — exercises the partial-final-byte path.
        let v = make_bipolar(0x99, 17);
        let b64 = encode(&v);
        let back = decode(&b64, 17).unwrap();
        assert_eq!(v.as_slice(), back.as_slice());
        // Distinct seeds produce distinct encodings.
        let other = make_bipolar(0x9A, 17);
        let other_b64 = encode(&other);
        assert_ne!(b64, other_b64);
    }

    #[test]
    fn pattern_helpers_roundtrip() {
        let v = make_bipolar(0xCAFEBABE, 10_000);
        let p = pattern_from_vector(
            &v,
            "test-pattern",
            "round-trip check",
            vec!["test".into()],
            PatternOrigin::Curated,
        );
        assert_eq!(p.dim, 10_000);
        let back = pattern_to_vector(&p).unwrap();
        assert_eq!(v.as_slice(), back.as_slice());
    }

    #[test]
    fn decode_rejects_truncated_input() {
        // Encode 10000 dims, then ask the decoder for 10001 —
        // there aren't enough bits, so it should error.
        let v = make_bipolar(7, 10_000);
        let b64 = encode(&v);
        match decode(&b64, 10_001) {
            Err(HdcEncoderError::DimMismatch {
                declared: 10_001, ..
            }) => {}
            other => panic!("expected DimMismatch, got {:?}", other),
        }
    }

    #[test]
    fn decode_rejects_malformed_base64() {
        let err = decode("not!valid!base64", 10).unwrap_err();
        assert!(matches!(err, HdcEncoderError::Base64(_)));
    }

    #[test]
    fn cosine_similarity_preserved_through_roundtrip() {
        // Two distinct random vectors. After roundtrip, their
        // pairwise cosine should match (within float precision).
        let a = make_bipolar(0xA, 1024);
        let b = make_bipolar(0xB, 1024);
        let a_round = decode(&encode(&a), 1024).unwrap();
        let b_round = decode(&encode(&b), 1024).unwrap();
        let orig_sim = a.cosine_similarity(&b).unwrap();
        let round_sim = a_round.cosine_similarity(&b_round).unwrap();
        assert!(
            (orig_sim - round_sim).abs() < 1e-6,
            "cosine drifted: orig={orig_sim} round={round_sim}"
        );
    }
}
