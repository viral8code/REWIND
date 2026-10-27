//! SHA-2, HMAC-SHA-256, PBKDF2-HMAC-SHA-256, hex and constant-time comparison for `std.crypto`.
//!
//! HMAC (RFC 2104) and PBKDF2 (RFC 8018) are written directly on the sha2 crate so no new
//! dependency is needed. These functions only compute; callers enforce the size limits.

use sha2::{Digest, Sha256, Sha512};
use subtle::ConstantTimeEq;

const BLOCK: usize = 64;

pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub fn sha512(data: &[u8]) -> [u8; 64] {
    Sha512::digest(data).into()
}

pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&sha256(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.map(|b| b ^ 0x36));
    inner.update(message);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(inner);
    outer.finalize().into()
}

/// `iterations` and `length` must already be in range (iterations >= 1, length >= 1).
pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32, length: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(length);
    let mut index = 1u32;
    while out.len() < length {
        let mut seed = salt.to_vec();
        seed.extend_from_slice(&index.to_be_bytes());
        let mut u = hmac_sha256(password, &seed);
        let mut t = u;
        for _ in 1..iterations {
            u = hmac_sha256(password, &u);
            for (a, b) in t.iter_mut().zip(u) {
                *a ^= b;
            }
        }
        let take = (length - out.len()).min(32);
        out.extend_from_slice(&t[..take]);
        index += 1;
    }
    out
}

/// Lowercase hexadecimal, two digits per byte.
pub fn to_hex(data: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(data.len() * 2);
    for byte in data {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

/// Accepts upper and lower case digits. On error returns the byte offset of the first bad
/// digit, or the text length when the number of digits is odd.
pub fn from_hex(text: &str) -> Result<Vec<u8>, usize> {
    let digits = text.as_bytes();
    if digits.len() % 2 != 0 {
        return Err(digits.len());
    }
    let mut out = Vec::with_capacity(digits.len() / 2);
    for (index, pair) in digits.chunks(2).enumerate() {
        let high = hex_value(pair[0]).ok_or(index * 2)?;
        let low = hex_value(pair[1]).ok_or(index * 2 + 1)?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

/// Unequal lengths return false immediately; the length itself is not treated as secret.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && bool::from(a.ct_eq(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        to_hex(bytes)
    }

    #[test]
    fn sha2_known_answers() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha512(b"abc")),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
    }

    #[test]
    fn hmac_sha256_rfc4231_vectors() {
        // RFC 4231 test case 1.
        assert_eq!(
            hex(&hmac_sha256(&[0x0b; 20], b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // RFC 4231 test case 2: a key shorter than the block size.
        assert_eq!(
            hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // RFC 4231 test case 6: a 131-byte key is hashed before use.
        assert_eq!(
            hex(&hmac_sha256(
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn pbkdf2_sha256_vectors() {
        // RFC 7914 section 11 and the widely published PBKDF2-HMAC-SHA256 vectors.
        assert_eq!(
            hex(&pbkdf2_sha256(b"password", b"salt", 1, 32)),
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
        assert_eq!(
            hex(&pbkdf2_sha256(b"password", b"salt", 2, 32)),
            "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
        );
        assert_eq!(
            hex(&pbkdf2_sha256(b"password", b"salt", 4096, 32)),
            "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
        );
        // Output longer than one 32-byte block, and a truncated final block.
        assert_eq!(pbkdf2_sha256(b"password", b"salt", 1, 40).len(), 40);
        assert_eq!(
            hex(&pbkdf2_sha256(b"password", b"salt", 1, 40)[..32]),
            hex(&pbkdf2_sha256(b"password", b"salt", 1, 32))
        );
    }

    #[test]
    fn hex_round_trip_and_errors() {
        assert_eq!(to_hex(&[0x00, 0xab, 0xff]), "00abff");
        assert_eq!(from_hex("00ABff").unwrap(), vec![0x00, 0xab, 0xff]);
        assert_eq!(from_hex("abc"), Err(3));
        assert_eq!(from_hex("zz"), Err(0));
        assert_eq!(from_hex("0g"), Err(1));
    }

    #[test]
    fn constant_time_eq_handles_lengths() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}
