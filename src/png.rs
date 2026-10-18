//! Bounded PNG decode/encode for `std.image`.
//!
//! Supported: 8-bit gray, gray+alpha, RGB and RGBA, no interlacing, any number of IDAT chunks.
//! Everything else is refused with `Unsupported`. Every allocation is bounded by the caller's pixel
//! limit before the pixel data is inflated, chunk CRCs are verified, and declared sizes are never
//! trusted: the zlib stream must produce exactly the number of bytes the header implies.

pub const MAX_DIMENSION: usize = 16384;
const MAX_CHUNKS: usize = 4096;
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

#[derive(Debug, PartialEq, Eq)]
pub enum PngError {
    /// An argument is outside its documented range.
    Range,
    /// The image or the data is larger than the limit given.
    Limit,
    /// A valid PNG feature this decoder does not implement (16-bit, palette, interlace, ...).
    Unsupported,
    /// The data ends before the image is complete; the offset is where more data was needed.
    Truncated(usize),
    /// The data is not a valid PNG; the offset is the start of the offending chunk or byte.
    Corrupt(usize),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Decoded {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub pixels: Vec<u8>,
}

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for part in parts {
        for byte in *part {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
            }
        }
    }
    !crc
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i32::from(a), i32::from(b), i32::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// `max_pixels` bounds width*height; `max_bytes` bounds the decoded byte count.
pub fn decode(data: &[u8], max_pixels: usize, max_bytes: usize) -> Result<Decoded, PngError> {
    if data.len() < SIGNATURE.len() {
        return Err(if SIGNATURE.starts_with(data) {
            PngError::Truncated(data.len())
        } else {
            PngError::Corrupt(0)
        });
    }
    if data[..8] != SIGNATURE {
        return Err(PngError::Corrupt(0));
    }
    let mut pos = 8;
    let mut header: Option<(usize, usize, usize, usize)> = None; // width, height, channels, color type
    let mut idat: Vec<u8> = Vec::new();
    let mut idat_open = false;
    let mut idat_done = false;
    let mut ended = false;
    let mut chunks = 0;
    while pos < data.len() && !ended {
        chunks += 1;
        if chunks > MAX_CHUNKS {
            return Err(PngError::Limit);
        }
        if pos + 8 > data.len() {
            return Err(PngError::Truncated(pos));
        }
        let length = be32(&data[pos..]) as usize;
        if length > 0x7fff_ffff {
            return Err(PngError::Corrupt(pos));
        }
        let kind = &data[pos + 4..pos + 8];
        if !kind.iter().all(u8::is_ascii_alphabetic) {
            return Err(PngError::Corrupt(pos));
        }
        let end = pos
            .checked_add(12)
            .and_then(|v| v.checked_add(length))
            .ok_or(PngError::Corrupt(pos))?;
        if end > data.len() {
            return Err(PngError::Truncated(pos));
        }
        let body = &data[pos + 8..pos + 8 + length];
        if be32(&data[pos + 8 + length..]) != crc32(&[kind, body]) {
            return Err(PngError::Corrupt(pos));
        }
        if header.is_none() && kind != b"IHDR" {
            return Err(PngError::Corrupt(pos));
        }
        match kind {
            b"IHDR" => {
                if header.is_some() || length != 13 {
                    return Err(PngError::Corrupt(pos));
                }
                let (w, h) = (be32(body) as usize, be32(&body[4..]) as usize);
                let (depth, color, compression, filter, interlace) =
                    (body[8], body[9], body[10], body[11], body[12]);
                if w == 0 || h == 0 || compression != 0 || filter != 0 || interlace > 1 {
                    return Err(PngError::Corrupt(pos));
                }
                let channels = match color {
                    0 => 1,
                    2 => 3,
                    4 => 2,
                    6 => 4,
                    3 => return Err(PngError::Unsupported),
                    _ => return Err(PngError::Corrupt(pos)),
                };
                if depth != 8 || interlace != 0 {
                    return Err(PngError::Unsupported);
                }
                if w > MAX_DIMENSION || h > MAX_DIMENSION {
                    return Err(PngError::Limit);
                }
                let pixels = w * h;
                if pixels > max_pixels || pixels * channels > max_bytes {
                    return Err(PngError::Limit);
                }
                header = Some((w, h, channels, usize::from(color)));
            }
            b"IDAT" => {
                if idat_done {
                    return Err(PngError::Corrupt(pos));
                }
                idat_open = true;
                let (w, h, channels, _) = header.expect("checked above");
                let raw = (w * channels + 1) * h;
                // A zlib stream of `raw` bytes cannot reasonably need more than raw plus framing.
                if idat.len() + length > raw + raw / 100 + 1024 {
                    return Err(PngError::Corrupt(pos));
                }
                idat.extend_from_slice(body);
            }
            b"IEND" => {
                if length != 0 {
                    return Err(PngError::Corrupt(pos));
                }
                ended = true;
                if end != data.len() {
                    return Err(PngError::Corrupt(end));
                }
            }
            b"PLTE" => {
                let color = header.expect("checked above").3;
                if color == 0 || color == 4 || length % 3 != 0 || length == 0 {
                    return Err(PngError::Corrupt(pos));
                }
            }
            other => {
                // Ancillary chunks (lowercase first letter) are checksummed and skipped;
                // an unknown critical chunk cannot be ignored safely.
                if other[0].is_ascii_uppercase() {
                    return Err(PngError::Unsupported);
                }
            }
        }
        if kind != b"IDAT" && idat_open {
            idat_done = true;
        }
        pos = end;
    }
    let Some((width, height, channels, _)) = header else {
        return Err(PngError::Truncated(data.len()));
    };
    if !ended || idat.is_empty() {
        return Err(PngError::Truncated(data.len()));
    }
    let row = width * channels;
    let raw_len = (row + 1) * height;
    let raw = match miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&idat, raw_len) {
        Ok(raw) => raw,
        Err(e) => {
            use miniz_oxide::inflate::TINFLStatus;
            return Err(match e.status {
                TINFLStatus::FailedCannotMakeProgress | TINFLStatus::NeedsMoreInput => {
                    PngError::Truncated(e.output.len())
                }
                _ => PngError::Corrupt(e.output.len()),
            });
        }
    };
    if raw.len() != raw_len {
        return Err(PngError::Truncated(raw.len()));
    }
    let mut pixels = vec![0u8; row * height];
    for y in 0..height {
        let filter = raw[y * (row + 1)];
        let src = &raw[y * (row + 1) + 1..(y + 1) * (row + 1)];
        let (before, current) = pixels.split_at_mut(y * row);
        let current = &mut current[..row];
        let previous: &[u8] = if y == 0 {
            &[]
        } else {
            &before[(y - 1) * row..]
        };
        for i in 0..row {
            let left = if i >= channels {
                current[i - channels]
            } else {
                0
            };
            let up = if y == 0 { 0 } else { previous[i] };
            let up_left = if y == 0 || i < channels {
                0
            } else {
                previous[i - channels]
            };
            current[i] = src[i].wrapping_add(match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => paeth(left, up, up_left),
                _ => return Err(PngError::Corrupt(y * (row + 1))),
            });
        }
    }
    Ok(Decoded {
        width,
        height,
        channels,
        pixels,
    })
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out.extend_from_slice(&crc32(&[kind, body]).to_be_bytes());
}

/// Encode 8-bit interleaved pixels. Each row uses the filter with the smallest sum of absolute
/// signed differences (the usual heuristic); the choice and the output are deterministic.
pub fn encode(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    max_bytes: usize,
) -> Result<Vec<u8>, PngError> {
    if width == 0 || height == 0 || !(1..=4).contains(&channels) {
        return Err(PngError::Range);
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(PngError::Limit);
    }
    let row = width * channels;
    if pixels.len() != row * height {
        return Err(PngError::Range);
    }
    if pixels.len() > max_bytes {
        return Err(PngError::Limit);
    }
    let mut raw = Vec::with_capacity((row + 1) * height);
    let mut candidate = [
        vec![0u8; row],
        vec![0u8; row],
        vec![0u8; row],
        vec![0u8; row],
        vec![0u8; row],
    ];
    for y in 0..height {
        let current = &pixels[y * row..(y + 1) * row];
        let previous: &[u8] = if y == 0 {
            &[]
        } else {
            &pixels[(y - 1) * row..y * row]
        };
        for i in 0..row {
            let left = if i >= channels {
                current[i - channels]
            } else {
                0
            };
            let up = if y == 0 { 0 } else { previous[i] };
            let up_left = if y == 0 || i < channels {
                0
            } else {
                previous[i - channels]
            };
            let value = current[i];
            candidate[0][i] = value;
            candidate[1][i] = value.wrapping_sub(left);
            candidate[2][i] = value.wrapping_sub(up);
            candidate[3][i] = value.wrapping_sub(((u16::from(left) + u16::from(up)) / 2) as u8);
            candidate[4][i] = value.wrapping_sub(paeth(left, up, up_left));
        }
        let best = (0..5)
            .min_by_key(|f| {
                candidate[*f]
                    .iter()
                    .map(|b| u64::from((*b as i8).unsigned_abs()))
                    .sum::<u64>()
            })
            .unwrap_or(0);
        raw.push(best as u8);
        raw.extend_from_slice(&candidate[best]);
    }
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(height as u32).to_be_bytes());
    // channel count -> PNG color type: 1 gray (0), 2 gray+alpha (4), 3 RGB (2), 4 RGBA (6)
    let color = match channels {
        1 => 0,
        2 => 4,
        3 => 2,
        _ => 6,
    };
    header.extend_from_slice(&[8, color, 0, 0, 0]);
    let mut out = SIGNATURE.to_vec();
    chunk(&mut out, b"IHDR", &header);
    chunk(
        &mut out,
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
    );
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(width: usize, height: usize, channels: usize) -> Vec<u8> {
        (0..width * height * channels)
            .map(|i| ((i * 7 + (i / width.max(1)) * 3) % 251) as u8)
            .collect()
    }

    #[test]
    fn round_trips_every_channel_count_and_filter_choice() {
        for channels in 1..=4 {
            for (w, h) in [(1, 1), (3, 5), (17, 9), (64, 64)] {
                let pixels = gradient(w, h, channels);
                let png = encode(&pixels, w, h, channels, 1 << 20).unwrap();
                let back = decode(&png, w * h, 1 << 20).unwrap();
                assert_eq!((back.width, back.height, back.channels), (w, h, channels));
                assert_eq!(back.pixels, pixels, "{w}x{h}x{channels}");
            }
        }
    }

    #[test]
    fn encoding_is_deterministic() {
        let pixels = gradient(40, 30, 3);
        assert_eq!(
            encode(&pixels, 40, 30, 3, 1 << 20),
            encode(&pixels, 40, 30, 3, 1 << 20)
        );
    }

    #[test]
    fn declared_size_above_the_limit_is_refused_before_inflating() {
        let png = encode(&gradient(32, 32, 4), 32, 32, 4, 1 << 20).unwrap();
        assert_eq!(decode(&png, 1023, 1 << 20), Err(PngError::Limit));
        assert_eq!(decode(&png, 1024, 4095), Err(PngError::Limit));
        assert!(decode(&png, 1024, 4096).is_ok());
    }

    #[test]
    fn every_truncation_is_reported_as_truncated_or_corrupt_never_ok() {
        let png = encode(&gradient(9, 7, 3), 9, 7, 3, 1 << 20).unwrap();
        for cut in 0..png.len() {
            match decode(&png[..cut], 1 << 20, 1 << 20) {
                Err(PngError::Truncated(_)) | Err(PngError::Corrupt(_)) => {}
                other => panic!("cut {cut}: {other:?}"),
            }
        }
        assert!(decode(&png, 1 << 20, 1 << 20).is_ok());
    }

    #[test]
    fn single_byte_corruption_is_detected_by_the_checksums() {
        let png = encode(&gradient(9, 7, 3), 9, 7, 3, 1 << 20).unwrap();
        for index in 0..png.len() {
            let mut damaged = png.clone();
            damaged[index] ^= 0x55;
            assert!(decode(&damaged, 1 << 20, 1 << 20).is_err(), "byte {index}");
        }
    }

    #[test]
    fn unsupported_features_and_trailing_data_are_rejected() {
        let mut png = encode(&gradient(2, 2, 3), 2, 2, 3, 1 << 20).unwrap();
        png.push(0);
        assert!(matches!(decode(&png, 16, 64), Err(PngError::Corrupt(_))));
        // 16-bit depth and palette color type in a hand-built, correctly checksummed header.
        for (depth, color) in [(16u8, 2u8), (8, 3), (1, 0)] {
            let mut out = SIGNATURE.to_vec();
            let mut ihdr = Vec::new();
            ihdr.extend_from_slice(&2u32.to_be_bytes());
            ihdr.extend_from_slice(&2u32.to_be_bytes());
            ihdr.extend_from_slice(&[depth, color, 0, 0, 0]);
            chunk(&mut out, b"IHDR", &ihdr);
            chunk(&mut out, b"IEND", &[]);
            assert_eq!(
                decode(&out, 16, 64),
                Err(PngError::Unsupported),
                "{depth}/{color}"
            );
        }
    }

    #[test]
    fn dimension_and_argument_limits() {
        assert_eq!(encode(&[], 0, 1, 1, 10), Err(PngError::Range));
        assert_eq!(encode(&[0; 3], 1, 1, 5, 10), Err(PngError::Range));
        assert_eq!(encode(&[0; 3], 1, 2, 3, 10), Err(PngError::Range));
        assert_eq!(encode(&[0; 3], 1, 1, 3, 2), Err(PngError::Limit));
        assert_eq!(
            encode(&[0; 3], MAX_DIMENSION + 1, 1, 1, 1 << 30),
            Err(PngError::Limit)
        );
    }

    #[test]
    fn a_decompression_bomb_in_idat_cannot_exceed_the_declared_size() {
        // 1x1 RGB image whose IDAT inflates to far more than the 4 bytes the header implies.
        let big = miniz_oxide::deflate::compress_to_vec_zlib(&vec![0u8; 1 << 20], 9);
        let mut out = SIGNATURE.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &big);
        chunk(&mut out, b"IEND", &[]);
        assert!(matches!(decode(&out, 16, 64), Err(PngError::Corrupt(_))));
    }
}
