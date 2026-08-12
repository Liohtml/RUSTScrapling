//! Small internal hashing helpers shared by fingerprinting, storage, and
//! the response cache.

/// Lowercase hex encoding of a byte slice (`{:02x}` per byte). Replaces the
/// `LowerHex` formatting that `sha2`'s digest output lost in the 0.10 → 0.11
/// upgrade; produces byte-identical strings to the old `format!("{:x}")`, so
/// persisted fingerprints, cache filenames, and storage keys stay stable.
pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_matches_the_old_lowerhex_formatting() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff, 0xa5]), "000fffa5");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn sha256_hex_is_stable() {
        // Pin the exact digest string so a future encoding change that would
        // invalidate persisted fingerprints/cache keys fails loudly.
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"stability");
        assert_eq!(
            hex(&hasher.finalize()),
            "b1e79532d5b97c775ed657e6aea1de1cb3728443c6b35ad80fa7c3c4ed0f4d1e"
        );
    }
}
