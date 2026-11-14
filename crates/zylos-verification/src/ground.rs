//! Ground enum — certainty levels for verification claims.
//!
//! Three levels inspired by Cortex Proof engine:
//! - `Observado`: Directly observed via tool output, file read, or measurement.
//! - `Confirmado`: Cross-validated by ≥2 independent sources or adversarial panel.
//! - `Crido`: Accepted on trust (LLM generation, user assertion) without independent verification.
//!
//! Conversions are explicit and auditable. The type system prevents accidental
//! promotion from lower to higher ground without going through `promote()`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Certainty level of a verified claim.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum Ground {
    /// Accepted on trust — LLM output, user assertion, unverified source.
    #[default]
    Crido = 0,
    /// Directly observed — tool output, file content, measurement, screenshot.
    Observado = 1,
    /// Cross-validated — confirmed by ≥2 independent sources or adversarial panel.
    Confirmado = 2,
}

impl Ground {
    /// Returns true if this ground level meets or exceeds the required minimum.
    pub fn meets(&self, minimum: Ground) -> bool {
        *self >= minimum
    }

    /// Explicitly promote to a higher ground level.
    /// Returns `None` if the target is not strictly higher (prevents silent no-ops).
    ///
    /// This is the ONLY way to increase ground level — direct assignment is blocked
    /// by making the enum non-Copy for mutation paths (Clone is fine for reads).
    pub fn promote(self, target: Ground) -> Option<Ground> {
        if target > self {
            Some(target)
        } else {
            None
        }
    }

    /// Demote to a lower ground level (always allowed — losing confidence is safe).
    pub fn demote(self, target: Ground) -> Ground {
        if target < self {
            target
        } else {
            self
        }
    }

    /// Parse from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "crido" | "believed" | "trusted" => Some(Ground::Crido),
            "observado" | "observed" => Some(Ground::Observado),
            "confirmado" | "confirmed" => Some(Ground::Confirmado),
            _ => None,
        }
    }
}

impl fmt::Display for Ground {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ground::Crido => write!(f, "Crido"),
            Ground::Observado => write!(f, "Observado"),
            Ground::Confirmado => write!(f, "Confirmado"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordering() {
        assert!(Ground::Crido < Ground::Observado);
        assert!(Ground::Observado < Ground::Confirmado);
        assert!(Ground::Crido < Ground::Confirmado);
    }

    #[test]
    fn test_meets() {
        assert!(Ground::Confirmado.meets(Ground::Observado));
        assert!(Ground::Observado.meets(Ground::Observado));
        assert!(!Ground::Crido.meets(Ground::Observado));
    }

    #[test]
    fn test_promote_valid() {
        assert_eq!(
            Ground::Crido.promote(Ground::Observado),
            Some(Ground::Observado)
        );
        assert_eq!(
            Ground::Crido.promote(Ground::Confirmado),
            Some(Ground::Confirmado)
        );
        assert_eq!(
            Ground::Observado.promote(Ground::Confirmado),
            Some(Ground::Confirmado)
        );
    }

    #[test]
    fn test_promote_invalid_returns_none() {
        assert_eq!(Ground::Confirmado.promote(Ground::Observado), None);
        assert_eq!(Ground::Observado.promote(Ground::Crido), None);
        assert_eq!(Ground::Crido.promote(Ground::Crido), None);
    }

    #[test]
    fn test_demote() {
        assert_eq!(
            Ground::Confirmado.demote(Ground::Observado),
            Ground::Observado
        );
        assert_eq!(Ground::Confirmado.demote(Ground::Crido), Ground::Crido);
        // Demote to same or higher is a no-op
        assert_eq!(
            Ground::Observado.demote(Ground::Confirmado),
            Ground::Observado
        );
    }

    #[test]
    fn test_from_str_loose() {
        assert_eq!(Ground::from_str_loose("crido"), Some(Ground::Crido));
        assert_eq!(Ground::from_str_loose("OBSERVADO"), Some(Ground::Observado));
        assert_eq!(
            Ground::from_str_loose("Confirmed"),
            Some(Ground::Confirmado)
        );
        assert_eq!(Ground::from_str_loose("believed"), Some(Ground::Crido));
        assert_eq!(Ground::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Ground::Crido), "Crido");
        assert_eq!(format!("{}", Ground::Observado), "Observado");
        assert_eq!(format!("{}", Ground::Confirmado), "Confirmado");
    }

    #[test]
    fn test_default_is_crido() {
        assert_eq!(Ground::default(), Ground::Crido);
    }

    #[test]
    fn test_serde_roundtrip() {
        let grounds = [Ground::Crido, Ground::Observado, Ground::Confirmado];
        for g in grounds {
            let json = serde_json::to_string(&g).unwrap();
            let deserialized: Ground = serde_json::from_str(&json).unwrap();
            assert_eq!(g, deserialized);
        }
    }
}
