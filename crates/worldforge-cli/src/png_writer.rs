//! A minimal RGB8 PNG writer.
//!
//! The plug-in never encodes PNG — the host owns the document. This exists only
//! so the CLI can emit previews, and it is written by hand to keep `png` out of
//! the workspace's shared dependency graph.

use std::fs;
use std::path::Path;

/// Writes `samples` (interleaved RGB, `0.0..=1.0`) as an 8-bit PNG.
pub fn write_rgb8(
    path: impl AsRef<Path>,
    samples: &[f32],
    width: u32,
    height: u32,
) -> Result<(), String> {
    let expected = (width as usize) * (height as usize) * 3;
    if samples.len() < expected {
        return Err(format!(
            "buffer holds {} samples, need {expected}",
            samples.len()
        ));
    }

    // Raw scanlines, each prefixed with filter type 0 (None).
    let mut raw = Vec::with_capacity(expected + height as usize);
    for row in 0..height as usize {
        raw.push(0u8);
        let start = row * width as usize * 3;
        for s in &samples[start..start + width as usize * 3] {
            let byte = (s.clamp(0.0, 1.0) * 255.0).round() as u8;
            raw.push(byte);
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(2); // colour type: truecolour
    ihdr.push(0); // deflate
    ihdr.push(0); // filter method 0
    ihdr.push(0); // no interlace
    write_chunk(&mut png, b"IHDR", &ihdr);

    // IDAT — stored (uncompressed) deflate blocks.
    let compressed = deflate_stored(&raw);
    write_chunk(&mut png, b"IDAT", &compressed);

    // IEND
    write_chunk(&mut png, b"IEND", &[]);

    if let Some(parent) = path.as_ref().parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    fs::write(path, png).map_err(|e| e.to_string())
}

/// Wraps `data` in zlib with stored (uncompressed) deflate blocks.
fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65_535 * 5 + 16);

    // zlib header: deflate, 32K window, no preset dictionary.
    out.push(0x78);
    out.push(0x01);

    let mut offset = 0;
    if data.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    while offset < data.len() {
        let chunk_len = (data.len() - offset).min(65_535);
        let is_final = offset + chunk_len >= data.len();

        out.push(if is_final { 0x01 } else { 0x00 });
        let len = chunk_len as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + chunk_len]);

        offset += chunk_len;
    }

    // Adler-32 of the uncompressed data.
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for byte in data {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());

    out
}

fn write_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);

    let mut crc = crc32(kind);
    crc = crc32_continue(crc, data);
    png.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    crc32_continue(0xFFFF_FFFF, data) ^ 0xFFFF_FFFF
}

fn crc32_continue(mut crc: u32, data: &[u8]) -> u32 {
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_known_value() {
        // The canonical check value for "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn png_has_a_valid_signature_and_chunks() {
        let samples = vec![0.5f32; 4 * 4 * 3];
        let dir = std::env::temp_dir().join("worldforge_png_test");
        let path = dir.join("test.png");

        write_rgb8(&path, &samples, 4, 4).expect("write");
        let bytes = std::fs::read(&path).expect("read");

        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
        // IHDR is always the first chunk.
        assert_eq!(&bytes[12..16], b"IHDR");
        // IEND is always last.
        assert_eq!(&bytes[bytes.len() - 8..bytes.len() - 4], b"IEND");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn short_buffer_is_rejected() {
        let err = write_rgb8(std::env::temp_dir().join("nope.png"), &[0.0; 4], 8, 8);
        assert!(err.is_err());
    }
}
