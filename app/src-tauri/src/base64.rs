//! Just enough base64 to put a PNG into an `<img src>`.
//!
//! Written rather than depended on. It is twenty lines, it has no versions to
//! track, and the alternative is another crate in the supply chain of a desktop
//! app for the sake of a lookup table.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The other direction, so a test can look at the picture the interface was
/// actually handed.
///
/// Test-only on purpose: nothing the app does needs to decode, and a function
/// that exists solely to be exercised by its own tests should say so rather
/// than sit in the public surface looking load-bearing.
///
/// Checked against the same RFC vectors as the encoder, in both directions,
/// rather than only against a round trip, since a round trip agrees with itself
/// whatever two mirrored mistakes it contains.
#[cfg(test)]
#[must_use]
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> { ALPHABET.iter().position(|a| *a == c).map(|i| i as u32) };

    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let symbols: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !symbols.len().is_multiple_of(4) {
        return None;
    }

    for quad in symbols.chunks(4) {
        let padding = quad.iter().filter(|c| **c == b'=').count();
        if padding > 2 {
            return None;
        }
        let mut n = 0u32;
        for c in quad {
            n = (n << 6) | if *c == b'=' { 0 } else { value(*c)? };
        }
        out.push((n >> 16) as u8);
        if padding < 2 {
            out.push((n >> 8) as u8);
        }
        if padding < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_rfc_examples_too() {
        // The same vectors read the other way, so the decoder is pinned to the
        // standard rather than to this file's encoder.
        for (bytes, text) in [
            (&b""[..], ""),
            (&b"f"[..], "Zg=="),
            (&b"fo"[..], "Zm8="),
            (&b"foo"[..], "Zm9v"),
            (&b"foob"[..], "Zm9vYg=="),
            (&b"fooba"[..], "Zm9vYmE="),
            (&b"foobar"[..], "Zm9vYmFy"),
        ] {
            assert_eq!(decode(text).as_deref(), Some(bytes), "decoding {text}");
        }
        assert_eq!(decode("AP+A").as_deref(), Some(&[0x00, 0xff, 0x80][..]));
    }

    #[test]
    fn refuses_what_is_not_base64() {
        assert_eq!(decode("Zg="), None, "length is not a multiple of four");
        assert_eq!(decode("Zg!="), None, "not an alphabet symbol");
    }

    #[test]
    fn matches_the_rfc_examples() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn handles_bytes_that_are_not_text() {
        assert_eq!(encode(&[0x00, 0xff, 0x80]), "AP+A");
    }
}
