//! Consistent per-instance browser fingerprint for anti-detect.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A stable browser fingerprint that persists across sessions for the same agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserFingerprint {
    /// Unique instance ID derived from agent identity.
    pub instance_id: String,
    /// Canvas hash (simulated).
    pub canvas_hash: String,
    /// WebGL vendor string.
    pub webgl_vendor: String,
    /// Screen resolution (width x height).
    pub screen_resolution: (u32, u32),
    /// Timezone offset in minutes.
    pub timezone_offset: i32,
    /// Language preference.
    pub language: String,
    /// User-agent string.
    pub user_agent: String,
}

impl BrowserFingerprint {
    /// Generate a deterministic fingerprint from an agent seed.
    /// Same seed always produces the same fingerprint.
    pub fn from_seed(seed: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        let hash = hex::encode(hasher.finalize());

        // Derive deterministic values from hash bytes
        let bytes = hex::decode(&hash[..16]).unwrap_or_default();
        let _rng = <rand::rngs::StdRng as rand::SeedableRng>::from_seed(
            <[u8; 32]>::try_from(hex::decode(&hash[..64]).unwrap_or_else(|_| vec![0u8; 32]))
                .unwrap_or([0u8; 32]),
        );

        let resolutions = [
            (1920, 1080),
            (2560, 1440),
            (1366, 768),
            (1440, 900),
            (1536, 864),
        ];
        let vendors = [
            "Google Inc. (NVIDIA)",
            "Google Inc. (Intel)",
            "Google Inc. (AMD)",
        ];
        let languages = ["en-US", "en-GB", "pt-BR", "de-DE", "fr-FR"];

        let res_idx = (bytes[0] as usize) % resolutions.len();
        let vendor_idx = (bytes[1] as usize) % vendors.len();
        let lang_idx = (bytes[2] as usize) % languages.len();

        Self {
            instance_id: hash[..16].to_string(),
            canvas_hash: hash[16..48].to_string(),
            webgl_vendor: vendors[vendor_idx].to_string(),
            screen_resolution: resolutions[res_idx],
            timezone_offset: ((bytes[3] as i32) - 128) * 15, // spread across timezones
            language: languages[lang_idx].to_string(),
            user_agent: format!(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36 ZylosBot/{}",
                &hash[..8]
            ),
        }
    }

    /// Compute a consistency hash to verify fingerprint hasn't changed between sessions.
    pub fn consistency_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.instance_id.as_bytes());
        hasher.update(self.canvas_hash.as_bytes());
        hasher.update(self.webgl_vendor.as_bytes());
        hasher.update(
            format!("{}x{}", self.screen_resolution.0, self.screen_resolution.1).as_bytes(),
        );
        hasher.update(self.timezone_offset.to_le_bytes());
        hasher.update(self.language.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_fingerprint() {
        let fp1 = BrowserFingerprint::from_seed("agent-alpha");
        let fp2 = BrowserFingerprint::from_seed("agent-alpha");
        assert_eq!(fp1.instance_id, fp2.instance_id);
        assert_eq!(fp1.canvas_hash, fp2.canvas_hash);
        assert_eq!(fp1.user_agent, fp2.user_agent);
    }

    #[test]
    fn test_different_seeds_produce_different_fingerprints() {
        let fp1 = BrowserFingerprint::from_seed("agent-alpha");
        let fp2 = BrowserFingerprint::from_seed("agent-beta");
        assert_ne!(fp1.instance_id, fp2.instance_id);
        assert_ne!(fp1.canvas_hash, fp2.canvas_hash);
    }

    #[test]
    fn test_consistency_hash_stable() {
        let fp = BrowserFingerprint::from_seed("agent-gamma");
        let h1 = fp.consistency_hash();
        let h2 = fp.consistency_hash();
        assert_eq!(h1, h2);
    }
}
