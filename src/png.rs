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

pub fn crc32(parts: &[&[u8]]) -> u32 {
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
    // width, height, channels stored per pixel in the file, color type, bit depth
    let mut header: Option<(usize, usize, usize, usize, usize)> = None;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut transparency: Vec<u8> = Vec::new();
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
                    3 => 1,
                    _ => return Err(PngError::Corrupt(pos)),
                };
                if color == 3 {
                    if !matches!(depth, 1 | 2 | 4 | 8) {
                        return Err(PngError::Corrupt(pos));
                    }
                } else if depth != 8 {
                    return Err(if matches!(depth, 1 | 2 | 4 | 16) {
                        PngError::Unsupported
                    } else {
                        PngError::Corrupt(pos)
                    });
                }
                if interlace != 0 {
                    return Err(PngError::Unsupported);
                }
                if w > MAX_DIMENSION || h > MAX_DIMENSION {
                    return Err(PngError::Limit);
                }
                let pixels = w * h;
                // A palette image decodes to RGB or RGBA; bound it by the larger.
                let output_channels = if color == 3 { 4 } else { channels };
                if pixels > max_pixels || pixels * output_channels > max_bytes {
                    return Err(PngError::Limit);
                }
                header = Some((w, h, channels, usize::from(color), usize::from(depth)));
            }
            b"IDAT" => {
                if idat_done {
                    return Err(PngError::Corrupt(pos));
                }
                idat_open = true;
                let (w, h, channels, color, depth) = header.expect("checked above");
                let raw = if color == 3 {
                    (usize::div_ceil(w * depth, 8) + 1) * h
                } else {
                    (w * channels + 1) * h
                };
                if color == 3 && palette.is_empty() {
                    return Err(PngError::Corrupt(pos));
                }
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
                let (_, _, _, color, depth) = header.expect("checked above");
                if color == 0
                    || color == 4
                    || length % 3 != 0
                    || length == 0
                    || length / 3 > 1 << depth.min(8)
                    || !palette.is_empty()
                    || idat_open
                {
                    return Err(PngError::Corrupt(pos));
                }
                palette = body.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
            }
            b"tRNS" => {
                let color = header.expect("checked above").3;
                if idat_open {
                    return Err(PngError::Corrupt(pos));
                }
                match color {
                    3 => {
                        if length > palette.len() || palette.is_empty() {
                            return Err(PngError::Corrupt(pos));
                        }
                        transparency = body.to_vec();
                    }
                    // Color-key transparency for gray/RGB is checked and ignored: such images
                    // decode without an alpha channel.
                    0 | 2 => {}
                    _ => return Err(PngError::Corrupt(pos)),
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
    let Some((width, height, channels, color, depth)) = header else {
        return Err(PngError::Truncated(data.len()));
    };
    if !ended || idat.is_empty() {
        return Err(PngError::Truncated(data.len()));
    }
    // `row` is the filtered row size in bytes; `bpp` the filter's byte distance to the left sample.
    let row = if color == 3 {
        usize::div_ceil(width * depth, 8)
    } else {
        width * channels
    };
    let bpp = if color == 3 { 1 } else { channels };
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
            let left = if i >= bpp { current[i - bpp] } else { 0 };
            let up = if y == 0 { 0 } else { previous[i] };
            let up_left = if y == 0 || i < bpp {
                0
            } else {
                previous[i - bpp]
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
    if color != 3 {
        return Ok(Decoded {
            width,
            height,
            channels,
            pixels,
        });
    }
    // Expand palette indices (1, 2, 4 or 8 bits, most significant bit first) to RGB or RGBA.
    let out_channels = if transparency.is_empty() { 3 } else { 4 };
    let mut expanded = Vec::with_capacity(width * height * out_channels);
    for y in 0..height {
        for x in 0..width {
            let bit = x * depth;
            let byte = pixels[y * row + bit / 8];
            let index =
                usize::from((byte >> (8 - depth - bit % 8)) & (((1u16 << depth) - 1) as u8));
            let Some(rgb) = palette.get(index) else {
                return Err(PngError::Corrupt(y * (row + 1)));
            };
            expanded.extend_from_slice(rgb);
            if out_channels == 4 {
                expanded.push(transparency.get(index).copied().unwrap_or(255));
            }
        }
    }
    Ok(Decoded {
        width,
        height,
        channels: out_channels,
        pixels: expanded,
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
        for (depth, color) in [(16u8, 2u8), (1, 0), (2, 6)] {
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

    /// Hand-built palette PNG: one filter-0 row per entry of `rows`, indices packed MSB first.
    fn palette_png(
        width: usize,
        depth: usize,
        rows: &[Vec<u8>],
        palette: &[[u8; 3]],
        alpha: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut raw = Vec::new();
        for row in rows {
            raw.push(0);
            let mut packed = vec![0u8; usize::div_ceil(width * depth, 8)];
            for (x, index) in row.iter().enumerate() {
                let bit = x * depth;
                packed[bit / 8] |= index << (8 - depth - bit % 8);
            }
            raw.extend_from_slice(&packed);
        }
        let mut out = SIGNATURE.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(width as u32).to_be_bytes());
        ihdr.extend_from_slice(&(rows.len() as u32).to_be_bytes());
        ihdr.extend_from_slice(&[depth as u8, 3, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        let plte: Vec<u8> = palette.iter().flatten().copied().collect();
        chunk(&mut out, b"PLTE", &plte);
        if let Some(alpha) = alpha {
            chunk(&mut out, b"tRNS", alpha);
        }
        chunk(
            &mut out,
            b"IDAT",
            &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
        );
        chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn palette_images_expand_for_every_bit_depth() {
        let colors: Vec<[u8; 3]> = (0..=255u8).map(|i| [i, 255 - i, i / 2]).collect();
        for depth in [1usize, 2, 4, 8] {
            let entries = 1usize << depth;
            for width in [1usize, 3, 7, 9] {
                let rows: Vec<Vec<u8>> = (0..3)
                    .map(|y| (0..width).map(|x| ((x * 3 + y) % entries) as u8).collect())
                    .collect();
                let png = palette_png(width, depth, &rows, &colors[..entries], None);
                let image = decode(&png, 1 << 16, 1 << 20).unwrap();
                assert_eq!((image.width, image.height, image.channels), (width, 3, 3));
                for (y, row) in rows.iter().enumerate() {
                    for (x, index) in row.iter().enumerate() {
                        let at = (y * width + x) * 3;
                        assert_eq!(
                            &image.pixels[at..at + 3],
                            &colors[usize::from(*index)],
                            "{depth}/{width}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn palette_transparency_gives_rgba_and_defaults_to_opaque() {
        let colors = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
        let rows = vec![vec![0u8, 1, 2]];
        let png = palette_png(3, 2, &rows, &colors, Some(&[0, 128]));
        let image = decode(&png, 16, 64).unwrap();
        assert_eq!(image.channels, 4);
        assert_eq!(image.pixels, vec![1, 2, 3, 0, 4, 5, 6, 128, 7, 8, 9, 255]);
    }

    #[test]
    fn invalid_palettes_are_corrupt_not_panics() {
        let colors = [[1, 2, 3], [4, 5, 6]];
        // index 3 does not exist in a 2-entry palette
        let bad_index = palette_png(2, 2, &[vec![0, 3]], &colors, None);
        assert!(matches!(
            decode(&bad_index, 16, 64),
            Err(PngError::Corrupt(_))
        ));
        // transparency longer than the palette
        let long_alpha = palette_png(2, 2, &[vec![0, 1]], &colors, Some(&[1, 2, 3]));
        assert!(matches!(
            decode(&long_alpha, 16, 64),
            Err(PngError::Corrupt(_))
        ));
        // more palette entries than the bit depth can address
        let big = palette_png(1, 1, &[vec![0]], &[[0; 3], [1; 3], [2; 3]], None);
        assert!(matches!(decode(&big, 16, 64), Err(PngError::Corrupt(_))));
        // palette color type without a PLTE chunk
        let mut out = SIGNATURE.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(
            &mut out,
            b"IDAT",
            &miniz_oxide::deflate::compress_to_vec_zlib(&[0, 0], 6),
        );
        chunk(&mut out, b"IEND", &[]);
        assert!(matches!(decode(&out, 16, 64), Err(PngError::Corrupt(_))));
        // palette images are limited by their RGBA size
        let ok = palette_png(4, 8, &[vec![0, 1, 0, 1]], &colors, None);
        assert_eq!(decode(&ok, 3, 1 << 20), Err(PngError::Limit));
        assert_eq!(decode(&ok, 4, 15), Err(PngError::Limit));
        assert!(decode(&ok, 4, 16).is_ok());
    }
}
