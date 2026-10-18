//! Bounded single-member gzip (RFC 1952) on top of miniz_oxide's raw DEFLATE.
//!
//! Compression writes a minimal header (no name, no timestamp, OS = unknown) so the output is
//! reproducible. Decompression accepts the optional header fields but only one member; anything
//! after the trailer is a CRC mismatch, never silently ignored.

use crate::png::crc32;

#[derive(Debug, PartialEq, Eq)]
pub enum GzipError {
    Range,
    Limit,
    Unsupported,
    Truncated(usize),
    Corrupt(usize),
}

const MAX_FIELD: usize = 4096;

pub fn compress(data: &[u8], level: u8) -> Vec<u8> {
    let body = miniz_oxide::deflate::compress_to_vec(data, level);
    let mut out = Vec::with_capacity(body.len() + 18);
    out.extend_from_slice(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255]);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&[data]).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

fn skip_zero_terminated(data: &[u8], pos: usize) -> Result<usize, GzipError> {
    let window = &data[pos.min(data.len())..];
    match window.iter().take(MAX_FIELD).position(|b| *b == 0) {
        Some(n) => Ok(pos + n + 1),
        None if window.len() < MAX_FIELD => Err(GzipError::Truncated(data.len())),
        None => Err(GzipError::Corrupt(pos)),
    }
}

pub fn decompress(data: &[u8], max_output: usize) -> Result<Vec<u8>, GzipError> {
    if data.len() < 10 {
        let prefix = [0x1f, 0x8b, 8];
        return Err(if data.iter().zip(prefix).all(|(a, b)| *a == b) {
            GzipError::Truncated(data.len())
        } else {
            GzipError::Corrupt(0)
        });
    }
    if data[0] != 0x1f || data[1] != 0x8b {
        return Err(GzipError::Corrupt(0));
    }
    if data[2] != 8 {
        return Err(GzipError::Unsupported);
    }
    let flags = data[3];
    if flags & 0xe0 != 0 {
        return Err(GzipError::Corrupt(3));
    }
    let mut pos = 10;
    if flags & 4 != 0 {
        if pos + 2 > data.len() {
            return Err(GzipError::Truncated(data.len()));
        }
        let extra = usize::from(u16::from_le_bytes([data[pos], data[pos + 1]]));
        pos += 2 + extra;
        if pos > data.len() {
            return Err(GzipError::Truncated(data.len()));
        }
    }
    if flags & 8 != 0 {
        pos = skip_zero_terminated(data, pos)?;
    }
    if flags & 16 != 0 {
        pos = skip_zero_terminated(data, pos)?;
    }
    if flags & 2 != 0 {
        if pos + 2 > data.len() {
            return Err(GzipError::Truncated(data.len()));
        }
        let stored = u16::from_le_bytes([data[pos], data[pos + 1]]);
        if stored != (crc32(&[&data[..pos]]) & 0xffff) as u16 {
            return Err(GzipError::Corrupt(pos));
        }
        pos += 2;
    }
    if pos + 8 > data.len() {
        return Err(GzipError::Truncated(data.len()));
    }
    let trailer = data.len() - 8;
    let out =
        match miniz_oxide::inflate::decompress_to_vec_with_limit(&data[pos..trailer], max_output) {
            Ok(out) => out,
            Err(e) => {
                use miniz_oxide::inflate::TINFLStatus;
                return Err(match e.status {
                    TINFLStatus::HasMoreOutput => GzipError::Limit,
                    TINFLStatus::FailedCannotMakeProgress | TINFLStatus::NeedsMoreInput => {
                        GzipError::Truncated(e.output.len())
                    }
                    _ => GzipError::Corrupt(e.output.len()),
                });
            }
        };
    let crc = u32::from_le_bytes([
        data[trailer],
        data[trailer + 1],
        data[trailer + 2],
        data[trailer + 3],
    ]);
    let size = u32::from_le_bytes([
        data[trailer + 4],
        data[trailer + 5],
        data[trailer + 6],
        data[trailer + 7],
    ]);
    if crc != crc32(&[&out]) || size != out.len() as u32 {
        return Err(GzipError::Corrupt(trailer));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_is_reproducible() {
        for len in [0usize, 1, 100, 70000] {
            let data: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
            let packed = compress(&data, 6);
            assert_eq!(packed, compress(&data, 6));
            assert_eq!(decompress(&packed, len).unwrap(), data);
        }
    }

    #[test]
    fn limit_truncation_and_corruption_are_distinguished() {
        let data = vec![7u8; 5000];
        let packed = compress(&data, 9);
        assert_eq!(decompress(&packed, 4999), Err(GzipError::Limit));
        for cut in 0..packed.len() {
            assert!(
                matches!(
                    decompress(&packed[..cut], 5000),
                    Err(GzipError::Truncated(_)) | Err(GzipError::Corrupt(_))
                ),
                "cut {cut}"
            );
        }
        // Bytes 4..10 (mtime, extra flags, OS) are informational and covered by no checksum.
        for index in (0..4).chain(10..packed.len()) {
            let mut damaged = packed.clone();
            damaged[index] ^= 0x41;
            assert!(decompress(&damaged, 5000).is_err(), "byte {index}");
        }
        let mut trailing = packed.clone();
        trailing.push(0);
        assert!(decompress(&trailing, 5000).is_err());
    }

    #[test]
    fn optional_header_fields_are_skipped() {
        let body = miniz_oxide::deflate::compress_to_vec(b"hello", 6);
        let mut data = vec![0x1f, 0x8b, 8, 4 | 8 | 16, 0, 0, 0, 0, 0, 3];
        data.extend_from_slice(&[2, 0, 9, 9]); // FEXTRA, 2 bytes
        data.extend_from_slice(b"name\0");
        data.extend_from_slice(b"comment\0");
        data.extend_from_slice(&body);
        data.extend_from_slice(&crc32(&[b"hello"]).to_le_bytes());
        data.extend_from_slice(&5u32.to_le_bytes());
        assert_eq!(decompress(&data, 5).unwrap(), b"hello");
    }
}
