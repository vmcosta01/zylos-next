//! Adapter between `zylos_verification::Ground` (3-level) and `zylos_proof::Ground` (5-level).
//!
//! Mapping:
//! - verification::Crido      → proof::Assumed   (trust-only, no observation)
//! - verification::Observado  → proof::Observed   (directly seen)
//! - verification::Confirmado → proof::Confirmed  (cross-validated)
//!
//! Reverse mapping loses precision for Verified/Proven (both map to Confirmado),
//! which is safe — demotion never overclaims certainty.

use crate::ground::Ground as VerifGround;

/// The 5-level ground from zylos-proof, re-exported here so downstream crates
/// don't need to depend on both types directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ProofGround {
    Assumed = 0,
    Observed = 1,
    Confirmed = 2,
    Verified = 3,
    Proven = 4,
}

impl From<VerifGround> for ProofGround {
    fn from(v: VerifGround) -> Self {
        match v {
            VerifGround::Crido => ProofGround::Assumed,
            VerifGround::Observado => ProofGround::Observed,
            VerifGround::Confirmado => ProofGround::Confirmed,
        }
    }
}

impl From<ProofGround> for VerifGround {
    fn from(p: ProofGround) -> Self {
        match p {
            ProofGround::Assumed => VerifGround::Crido,
            ProofGround::Observed => VerifGround::Observado,
            // Verified and Proven are higher than Confirmado but we can't represent them;
            // safe demotion to highest available level.
            ProofGround::Confirmed | ProofGround::Verified | ProofGround::Proven => {
                VerifGround::Confirmado
            }
        }
    }
}

impl ProofGround {
    /// Convert from the raw u8 used in zylos-proof's wire format.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(ProofGround::Assumed),
            1 => Some(ProofGround::Observed),
            2 => Some(ProofGround::Confirmed),
            3 => Some(ProofGround::Verified),
            4 => Some(ProofGround::Proven),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_mapping() {
        assert_eq!(ProofGround::from(VerifGround::Crido), ProofGround::Assumed);
        assert_eq!(
            ProofGround::from(VerifGround::Observado),
            ProofGround::Observed
        );
        assert_eq!(
            ProofGround::from(VerifGround::Confirmado),
            ProofGround::Confirmed
        );
    }

    #[test]
    fn reverse_mapping_exact() {
        assert_eq!(VerifGround::from(ProofGround::Assumed), VerifGround::Crido);
        assert_eq!(
            VerifGround::from(ProofGround::Observed),
            VerifGround::Observado
        );
        assert_eq!(
            VerifGround::from(ProofGround::Confirmed),
            VerifGround::Confirmado
        );
    }

    #[test]
    fn reverse_mapping_demotes_higher_levels() {
        assert_eq!(
            VerifGround::from(ProofGround::Verified),
            VerifGround::Confirmado
        );
        assert_eq!(
            VerifGround::from(ProofGround::Proven),
            VerifGround::Confirmado
        );
    }

    #[test]
    fn roundtrip_lossless_for_three_levels() {
        for v in [
            VerifGround::Crido,
            VerifGround::Observado,
            VerifGround::Confirmado,
        ] {
            let p: ProofGround = v.into();
            let back: VerifGround = p.into();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn u8_roundtrip() {
        for byte in 0..=4u8 {
            let pg = ProofGround::from_u8(byte).unwrap();
            assert_eq!(pg.as_u8(), byte);
        }
        assert_eq!(ProofGround::from_u8(5), None);
    }
}
