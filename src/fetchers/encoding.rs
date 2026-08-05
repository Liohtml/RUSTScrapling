//! Charset detection and body decoding for HTTP responses.
//!
//! Responses are decoded according to the charset advertised in the
//! `Content-Type` header instead of assuming UTF-8, so pages whose header
//! declares ISO-8859-1, windows-1252, Shift_JIS, etc. no longer turn into
//! mojibake. Pages that only declare their charset in a `<meta>` tag are
//! still decoded as lossy UTF-8.

use regex::Regex;
use std::sync::LazyLock;

/// Matches the charset parameter of a `Content-Type` header. RFC 7231 allows
/// the parameter value to be a quoted-string (e.g. `charset="ISO-8859-1"`),
/// so an optional surrounding quote is consumed before capturing the label.
/// The parameter must start at the beginning of the header or right after a
/// `;` so `x-charset=...` or a `charset=` inside another parameter's quoted
/// value (e.g. a multipart boundary) cannot match; whitespace around `=` is
/// tolerated since misconfigured servers commonly emit it.
static CHARSET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:^|;)\s*charset\s*=\s*["']?([\w-]+)"#).expect("charset regex is valid")
});

/// Extract the charset label from a `Content-Type` header value, if present.
///
/// Handles both bare (`charset=utf-8`) and quoted (`charset="ISO-8859-1"`)
/// parameter forms; the quotes are not part of the returned label.
#[must_use]
pub fn charset_from_content_type(content_type: &str) -> Option<&str> {
    CHARSET_RE
        .captures(content_type)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
}

/// Decode a response body using the charset from the `Content-Type` header.
///
/// Unknown or missing charsets fall back to UTF-8 with lossy replacement. A
/// byte-order mark in the body takes precedence over the header, matching
/// the WHATWG encoding standard behaviour.
///
/// `for_label_no_replacement` is used because the WHATWG spec maps a few
/// legacy labels (`hz-gb-2312`, `iso-2022-kr`, …) to the *replacement*
/// encoding, which decodes the entire body to a single U+FFFD — for a
/// scraper, falling back to lossy UTF-8 preserves far more of the content.
#[must_use]
pub fn decode_body(bytes: &[u8], content_type: &str) -> String {
    let encoding = charset_from_content_type(content_type)
        .and_then(|label| encoding_rs::Encoding::for_label_no_replacement(label.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_charset_is_extracted() {
        assert_eq!(
            charset_from_content_type("text/html; charset=utf-8"),
            Some("utf-8")
        );
    }

    #[test]
    fn double_quoted_charset_is_extracted() {
        assert_eq!(
            charset_from_content_type(r#"text/html; charset="ISO-8859-1""#),
            Some("ISO-8859-1")
        );
    }

    #[test]
    fn single_quoted_charset_is_extracted() {
        assert_eq!(
            charset_from_content_type("text/html; charset='windows-1252'"),
            Some("windows-1252")
        );
    }

    #[test]
    fn charset_is_case_insensitive() {
        assert_eq!(
            charset_from_content_type("text/html; CHARSET=UTF-8"),
            Some("UTF-8")
        );
    }

    #[test]
    fn missing_charset_returns_none() {
        assert_eq!(charset_from_content_type("text/html"), None);
        assert_eq!(charset_from_content_type(""), None);
    }

    #[test]
    fn whitespace_around_equals_is_tolerated() {
        assert_eq!(
            charset_from_content_type("text/html; charset = ISO-8859-1"),
            Some("ISO-8859-1")
        );
        assert_eq!(
            charset_from_content_type("text/html;charset=utf-8"),
            Some("utf-8")
        );
    }

    #[test]
    fn other_parameter_names_do_not_match() {
        // `x-charset` is a different parameter, not a charset declaration.
        assert_eq!(
            charset_from_content_type("text/html; x-charset=utf-16be"),
            None
        );
        // A `charset=` inside another parameter's quoted value must not match.
        assert_eq!(
            charset_from_content_type(r#"multipart/mixed; boundary="==charset=utf-16be==""#),
            None
        );
    }

    #[test]
    fn replacement_encoding_labels_fall_back_to_utf8() {
        // WHATWG maps these legacy labels to the replacement encoding, which
        // would decode the whole body to a single U+FFFD. Falling back to
        // lossy UTF-8 keeps the (mostly ASCII) content readable instead.
        let body = b"<html>hello world</html>";
        assert_eq!(
            decode_body(body, "text/html; charset=hz-gb-2312"),
            "<html>hello world</html>"
        );
        assert_eq!(
            decode_body(body, "text/html; charset=iso-2022-kr"),
            "<html>hello world</html>"
        );
    }

    #[test]
    fn latin1_body_is_decoded() {
        // "café" in ISO-8859-1: the é is a single 0xE9 byte, invalid as UTF-8.
        let bytes = b"caf\xe9";
        assert_eq!(
            decode_body(bytes, "text/html; charset=ISO-8859-1"),
            "caf\u{e9}"
        );
    }

    #[test]
    fn quoted_latin1_charset_is_honoured() {
        // Regression for the quoted-charset case: previously the charset
        // failed to parse and the body fell back to lossy UTF-8.
        let bytes = b"caf\xe9";
        assert_eq!(
            decode_body(bytes, r#"text/html; charset="ISO-8859-1""#),
            "caf\u{e9}"
        );
    }

    #[test]
    fn shift_jis_body_is_decoded() {
        // "日本" encoded as Shift_JIS.
        let bytes = b"\x93\xfa\x96\x7b";
        assert_eq!(
            decode_body(bytes, "text/html; charset=Shift_JIS"),
            "\u{65e5}\u{672c}"
        );
    }

    #[test]
    fn unknown_charset_falls_back_to_utf8() {
        assert_eq!(
            decode_body("héllo".as_bytes(), "text/html; charset=bogus-enc"),
            "héllo"
        );
    }

    #[test]
    fn missing_content_type_falls_back_to_utf8() {
        assert_eq!(decode_body("plain".as_bytes(), ""), "plain");
    }

    #[test]
    fn invalid_utf8_without_charset_is_lossy() {
        let bytes = b"caf\xe9";
        assert_eq!(decode_body(bytes, "text/html"), "caf\u{fffd}");
    }

    #[test]
    fn utf8_bom_overrides_header_charset() {
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend_from_slice("café".as_bytes());
        assert_eq!(decode_body(&bytes, "text/html; charset=ISO-8859-1"), "café");
    }
}
