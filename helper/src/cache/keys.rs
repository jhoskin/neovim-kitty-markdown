//! Cache key formulas from AGENTS.md "Cache keys". The plugin computes the
//! same keys in Lua to predict hits, so the byte layout here is a contract:
//! lowercase hex, `|` between fields, `0` for an absent limit.

use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// `sha256(sha256(bytes) | width_px | max_h | max_w)`: the lowercase hex of
/// the inner digest, then `|`, then each number in decimal, with `0` for an
/// absent limit. The plugin builds the same ASCII string in Lua.
pub fn key_image(
    content_sha_hex: &str,
    width_px: u32,
    max_h: Option<u32>,
    max_w: Option<u32>,
) -> String {
    let input = format!(
        "{content_sha_hex}|{width_px}|{}|{}",
        max_h.unwrap_or(0),
        max_w.unwrap_or(0)
    );
    sha256_hex(input.as_bytes())
}

/// `key_image` for callers that hold the bytes rather than their hash.
pub fn key_image_bytes(
    bytes: &[u8],
    width_px: u32,
    max_h: Option<u32>,
    max_w: Option<u32>,
) -> String {
    key_image(&sha256_hex(bytes), width_px, max_h, max_w)
}

/// `sha256(url)`; the entry has no extension because decoders sniff.
pub fn key_remote(url: &str) -> String {
    sha256_hex(url.as_bytes())
}

/// `sha256(sha256(source) | width_px | theme | mermaid_version)`: the
/// lowercase hex of the source digest, then the fields as ASCII with `|`
/// between them.
pub fn key_mermaid(source: &str, width_px: u32, theme: &str, mermaid_version: &str) -> String {
    let input = format!(
        "{}|{width_px}|{theme}|{mermaid_version}",
        sha256_hex(source.as_bytes())
    );
    sha256_hex(input.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// Fixed vectors shared with the Lua tests (`tests/cache_spec.lua`), so
    /// both sides can prove they compute the same file names. The expected
    /// hex values were computed independently with Python's hashlib.
    #[test]
    fn fixed_vectors_match_the_lua_side() {
        let content_sha = "ab".repeat(32);
        assert_eq!(
            key_image(&content_sha, 640, None, None),
            "b832a6f4ff2531ff905546d7eb459c7add35d872a6f9564c8d5e2c1db20a7cbd"
        );
        assert_eq!(
            key_image(&content_sha, 800, Some(600), Some(1200)),
            "ef29e14132c4940901a69051a25cbd239284b407ef8f9dcb6b18a433c9cf2327"
        );
        assert_eq!(
            key_remote("https://example.com/a.png"),
            "494a30704d4f32ac0b81739d18a66d3638d440cbc6f5669f6af66f840edee5ab"
        );
        let source = "graph TD\n  A-->B\n";
        assert_eq!(
            sha256_hex(source.as_bytes()),
            "917423685aa27a50430374642e473b7f36ec781cb0773801e3917fd279a60b9b"
        );
        assert_eq!(
            key_mermaid(source, 640, "dark", "12.2.0"),
            "aad58e9641dc504e8f34b542c7039ea058354e9b33916f6a02933424c1d979fb"
        );
    }

    #[test]
    fn absent_limits_are_zero() {
        let sha = sha256_hex(b"png");
        assert_eq!(
            key_image(&sha, 640, None, None),
            sha256_hex(format!("{sha}|640|0|0").as_bytes())
        );
        assert_eq!(
            key_image(&sha, 640, Some(10), None),
            sha256_hex(format!("{sha}|640|10|0").as_bytes())
        );
    }
}
