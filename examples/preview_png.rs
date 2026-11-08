//! Renders preview PNGs for several seeds, to eyeball the range of variation.
//!
//! ```text
//! cargo run --example preview_png -- out/preview.png 512 123456
//! ```
//!
//! This is a development harness, not part of the plug-in: the host owns the
//! document and the plug-in writes samples in place.

use worldforge_core::{WorldConfig, land_fraction, render_image};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "out/preview.png".to_string());
    let size: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(512);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(123_456);

    let config = WorldConfig {
        seed,
        ..WorldConfig::default()
    };
    let samples = render_image(config, size, size, 3);

    write_png(&out, &samples, size, size)?;

    let land = land_fraction(config, 128);
    println!("wrote {out} ({size}x{size}, land {:.1}%)", land * 100.0);
    Ok(())
}

/// Minimal RGB8 PNG encoder (stored deflate), kept local to the example.
fn write_png(path: &str, samples: &[f32], width: u32, height: u32) -> std::io::Result<()> {
    use std::io::Write;

    let mut raw = Vec::new();
    for row in 0..height as usize {
        raw.push(0u8);
        let start = row * width as usize * 3;
        for s in &samples[start..start + width as usize * 3] {
            raw.push((s.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);

    // zlib + stored deflate blocks
    let mut z = vec![0x78, 0x01];
    let mut off = 0;
    while off < raw.len() {
        let n = (raw.len() - off).min(65_535);
        let last = off + n >= raw.len();
        z.push(if last { 1 } else { 0 });
        z.extend_from_slice(&(n as u16).to_le_bytes());
        z.extend_from_slice(&(!(n as u16)).to_le_bytes());
        z.extend_from_slice(&raw[off..off + n]);
        off += n;
    }
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for byte in &raw {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);

    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    file.write_all(&png)
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);

    let mut crc = crc(kind, 0xFFFF_FFFF) ^ 0xFFFF_FFFF;
    crc = crc_parts(data, crc) ^ 0xFFFF_FFFF;
    png.extend_from_slice(&crc.to_be_bytes());
}

fn crc_parts(data: &[u8], mut crc: u32) -> u32 {
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    crc
}

fn crc(kind: &[u8; 4], init: u32) -> u32 {
    crc_parts(kind, init)
}
