//! Standard base64 (RFC 4648 §4, with padding) for binary payloads in JSON: `doc.blob.get`, embedded blobs of
//! interchange bundles (10 §6.8).

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes bytes.
#[must_use]
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let idx = |shift: u32| usize::try_from((n >> shift) & 63).unwrap_or(0);
        out.push(char::from(ALPHABET[idx(18)]));
        out.push(char::from(ALPHABET[idx(12)]));
        out.push(if chunk.len() > 1 { char::from(ALPHABET[idx(6)]) } else { '=' });
        out.push(if chunk.len() > 2 { char::from(ALPHABET[idx(0)]) } else { '=' });
    }
    out
}

/// Decodes standard base64 (padding required, no white space). `None` for anything else.
#[must_use]
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let b = text.as_bytes();
    if b.len() % 4 != 0 {
        return None;
    }
    let val = |c: u8| -> Option<u32> {
        Some(u32::from(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        }))
    };
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    let groups = b.len() / 4;
    for (gi, g) in b.chunks(4).enumerate() {
        let last = gi + 1 == groups;
        let pad = g.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for (i, c) in g.iter().enumerate() {
            n <<= 6;
            if i < 4 - pad {
                n |= val(*c)?;
            } else if *c != b'=' {
                return None;
            }
        }
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..4 - pad]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4648_vectors() {
        for (plain, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode(plain.as_bytes()), enc);
            assert_eq!(decode(enc).as_deref(), Some(plain.as_bytes()));
        }
        for bad in ["Zg=", "Z===", "Zg==Zg==", "Zm9v!", "Zm 9v"] {
            assert_eq!(decode(bad), None, "{bad}");
        }
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(decode(&encode(&all)).as_deref(), Some(all.as_slice()));
    }
}
