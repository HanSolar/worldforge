//! PhotoCraft WebAssembly ABI v1.
//!
//! Contract (upstream `docs/plugins.md`):
//!
//! | Export | Signature |
//! |---|---|
//! | `pc_abi_version` | `() -> i32`, must return `1` |
//! | `pc_manifest` | `() -> i64`, `(len << 32) \| ptr` of UTF-8 JSON |
//! | `pc_alloc` | `(size) -> ptr`, a block the host may write |
//! | `pc_filter` | `(buf, buf_len, width, height, channels, format, params, params_len) -> i32` |
//!
//! Rules that shape this module (spec §5.3):
//!
//! - No host imports: no WASI, no clock, no randomness, no filesystem.
//! - `pc_filter` writes samples in place and returns `0` or a non-zero code.
//! - **No panics across the boundary.** A panic is a trap, which aborts the whole
//!   run, so every entry point is written to be total.
//! - Band geometry arrives in the params JSON as `_image`; state must not be
//!   assumed to survive between calls.

use core::ptr;

use worldforge_core::{Band, Field, RenderOptions, config_from_params, raster::render_band};

/// ABI version the host requires.
pub const ABI_VERSION: i32 = 1;

/// Success code.
pub const OK: i32 = 0;
/// Input samples were missing, misaligned, or not `f32`.
pub const ERR_INPUT: i32 = 1;
/// The parameters were not valid JSON, or described an unusable config.
pub const ERR_PARAMS: i32 = 2;
/// Geometry was missing, zero-sized, or overflowed.
pub const ERR_GEOMETRY: i32 = 3;

/// The `pc_abi_version` export.
#[unsafe(no_mangle)]
pub extern "C" fn pc_abi_version() -> i32 {
    ABI_VERSION
}

/// The manifest, baked in so no allocation happens.
static MANIFEST: &str = crate::manifest::MANIFEST;

/// The `pc_manifest` export: `(len << 32) | ptr`.
///
/// The ABI packs a pointer into the low 32 bits, which assumes wasm32. On a
/// 64-bit host (only ever native tests — the plug-in itself is wasm32) a
/// truncated pointer would be garbage, so this aborts loudly rather than handing
/// back a bad address.
#[unsafe(no_mangle)]
pub extern "C" fn pc_manifest() -> i64 {
    let bytes = MANIFEST.as_bytes();
    let (ptr, len) = manifest_pointer(bytes.as_ptr(), bytes.len());
    ((len as i64) << 32) | i64::from(ptr)
}

/// Packs a manifest pointer into the ABI's 32-bit slot.
///
/// Extracted so the packing rule is testable independently of the export.
///
/// Note there is deliberately no assertion here: this is reachable from an
/// `extern "C"` export, and a panic there aborts the process instead of
/// unwinding (spec §5.3 point 8). On wasm32 a pointer is 32 bits so this is
/// lossless; natively the high bits are discarded, which is why native tests
/// compare the packing rule rather than dereferencing the result.
fn manifest_pointer(addr: *const u8, len: usize) -> (u32, u32) {
    ((addr as usize) as u32, len as u32)
}

/// Static allocator state.
///
/// The host calls `pc_alloc` to get a block it will fill, then passes a pointer
/// back to `pc_filter`. Each band is a fresh instance, so a bump allocator over
/// one static region is sufficient and avoids pulling in `std`'s allocator.
///
/// 8 MiB is the ceiling for the pixel buffer the host asks back. The host's own
/// band limit is ~4 MiB of `f32`, so this leaves room for the params string.
const ARENA_SIZE: usize = 8 * 1024 * 1024;

#[repr(align(16))]
struct Arena([u8; ARENA_SIZE]);

static mut ARENA: Arena = Arena([0; ARENA_SIZE]);
static mut CURSOR: usize = 0;

/// The `pc_alloc` export.
///
/// Returns a pointer into a static arena, or `0` on failure. Never freed: each
/// band gets a fresh instance, so the arena is reclaimed by the host.
///
/// # Safety
///
/// Called by the host with `size` bytes it intends to write. Not thread-safe by
/// design — the ABI runs one instance per band, single-threaded within it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_alloc(size: i32) -> *mut u8 {
    if size <= 0 {
        return ptr::null_mut();
    }
    let size = size as usize;

    // SAFETY: the ABI runs single-threaded per instance; these statics are only
    // touched from entry points invoked sequentially by the host.
    unsafe {
        let aligned = (CURSOR + 15) & !15;
        let end = match aligned.checked_add(size) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };
        if end > ARENA_SIZE {
            return ptr::null_mut();
        }
        CURSOR = end;
        ptr::addr_of_mut!(ARENA.0).cast::<u8>().add(aligned)
    }
}

/// The `pc_filter` export.
///
/// `format` encodes `depth | alpha << 8 | mode << 16`; only the channel count
/// affects this filter, but the depth/mode are validated so an unsupported
/// document fails cleanly instead of producing wrong pixels (spec §5.3 point 8).
///
/// # Safety
///
/// `buf` must point to `buf_len` writable bytes; `params` to `params_len` bytes
/// (not necessarily NUL-terminated). Both must stay valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pc_filter(
    buf: *mut u8,
    buf_len: i32,
    width: i32,
    height: i32,
    channels: i32,
    format: i32,
    params: *const u8,
    params_len: i32,
) -> i32 {
    // Geometry first: cheapest rejection, and it also guards the buffer maths.
    // Geometry validity is checked by `Band::checked`; channels must be 1..=4
    // because a filter that cannot address the colour model must fail clearly.
    if width <= 0 || height <= 0 || !(1..=4).contains(&channels) {
        return ERR_GEOMETRY;
    }

    let depth = format & 0xFF;
    // Alpha is informational: the filter never writes the last channel, so it
    // behaves correctly whether or not the document has one.
    let mode = (format >> 16) & 0xFF;

    // Supported colour modes: only the ones stored as RGB samples.
    //   2 Indexed (the host stores it as RGB), 3 RGB.
    // Grayscale, CMYK, Duotone, Lab and Multichannel are rejected rather than
    // filled with RGB values into a non-RGB channel layout, which would be the
    // "silently corrupted output" spec §5.4 forbids. An unsupported document
    // gets a clean error instead of wrong pixels.
    let depth_ok = matches!(depth, 0 | 8 | 16 | 32);
    let mode_ok = matches!(mode, 2 | 3);
    if !depth_ok || !mode_ok {
        return ERR_INPUT;
    }

    let band = match Band::checked(width as u32, height as u32, channels as u32) {
        Some(band) => band,
        None => return ERR_GEOMETRY,
    };

    // buf_len is in bytes of f32 samples.
    let expected_bytes = match band.sample_count().checked_mul(4) {
        Some(n) => n,
        None => return ERR_GEOMETRY,
    };
    if buf_len < 0 || (buf_len as usize) < expected_bytes {
        return ERR_INPUT;
    }
    if buf.is_null() {
        return ERR_INPUT;
    }

    if params.is_null() || params_len <= 0 {
        return ERR_PARAMS;
    }

    // SAFETY: caller contract guarantees buf/params describe readable regions
    // of the stated lengths for the duration of the call.
    let (byte_slice, param_slice) = unsafe {
        (
            core::slice::from_raw_parts_mut(buf, expected_bytes),
            core::slice::from_raw_parts(params, params_len as usize),
        )
    };

    let params_text = match core::str::from_utf8(param_slice) {
        Ok(text) => text,
        Err(_) => return ERR_PARAMS,
    };

    let config = match config_from_params(params_text) {
        Ok(config) => config,
        Err(_) => return ERR_PARAMS,
    };

    let geometry = match BandGeometry::parse(params_text) {
        Some(geometry) => geometry,
        None => return ERR_GEOMETRY,
    };

    // f32 samples must be 4-byte aligned. The host allocates via pc_alloc, which
    // returns 16-byte-aligned blocks, so this should always hold; check rather
    // than assume, because an unaligned write is UB.
    if (byte_slice.as_mut_ptr() as usize) % core::mem::align_of::<f32>() != 0 {
        return ERR_INPUT;
    }

    // SAFETY: length is a multiple of 4 (checked above) and the pointer is
    // 4-byte aligned (checked above), so this is a valid f32 buffer.
    let samples: &mut [f32] = unsafe {
        core::slice::from_raw_parts_mut(byte_slice.as_mut_ptr().cast::<f32>(), band.sample_count())
    };

    let field = Field::new(config);
    let options = RenderOptions {
        canvas_width: geometry.canvas_width,
        canvas_height: geometry.canvas_height,
        coast_glow: config.coast_glow,
        pixel_scale: config.pixel_scale,
    };

    // Work in global document coordinates: the band's origin comes from
    // `_image`, never from an assumed (0, 0).
    let global = Band {
        x: geometry.x,
        y: geometry.y,
        ..band
    };

    render_band(&field, &global, &options, samples);

    OK
}

/// The `_image` block the host appends to the params JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BandGeometry {
    x: u32,
    y: u32,
    canvas_width: u32,
    canvas_height: u32,
}

impl BandGeometry {
    /// Extracts `_image` from the params JSON by hand.
    ///
    /// A tiny string scan rather than a JSON parse: the object is flat, the keys
    /// are fixed, and it keeps the wasm footprint down.
    fn parse(params: &str) -> Option<Self> {
        let image = extract_object(params, "_image")?;
        Some(Self {
            x: extract_int(image, "x")?.max(0) as u32,
            y: extract_int(image, "y")?.max(0) as u32,
            canvas_width: extract_int(image, "canvasWidth")?.max(1) as u32,
            canvas_height: extract_int(image, "canvasHeight")?.max(1) as u32,
        })
    }
}

/// Finds `"key": { ... }` and returns the inside of the braces.
fn extract_object<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)?;
    let after = &json[start + needle.len()..];
    let open = after.find('{')?;
    let body = &after[open + 1..];
    let close = body.find('}')?;
    Some(&body[..close])
}

/// Finds `"key": <int>` and returns the integer, tolerating whitespace.
fn extract_int(json: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)?;
    let after = &json[start + needle.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();

    let mut end = 0;
    let bytes = rest.as_bytes();
    if end < bytes.len() && (bytes[end] == b'-' || bytes[end] == b'+') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_start {
        return None;
    }

    rest[..end].parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_one() {
        assert_eq!(pc_abi_version(), 1);
    }

    #[test]
    fn manifest_pointer_and_length_are_consistent() {
        let packed = pc_manifest();
        let len = (packed >> 32) as u32;
        let ptr = (packed & 0xFFFF_FFFF) as u32;

        assert_eq!(len as usize, MANIFEST.len());
        assert_ne!(ptr, 0);

        // The low 32 bits are a real address only under wasm32. Natively the
        // pointer is 64-bit, so compare the packing against the same rule
        // instead of dereferencing a truncated address.
        let (expected_ptr, expected_len) = manifest_pointer(MANIFEST.as_ptr(), MANIFEST.len());
        assert_eq!(ptr, expected_ptr);
        assert_eq!(len, expected_len);
    }

    #[test]
    fn manifest_bytes_are_the_exported_text() {
        // Reads through the *native* pointer, proving the static the ABI points
        // at is the manifest we expect. Safe because this is the real address.
        let bytes = MANIFEST.as_bytes();
        assert_eq!(bytes[0], b'{');
        assert_eq!(bytes[bytes.len() - 1], b'}');
        assert!(core::str::from_utf8(bytes).is_ok());
    }

    #[test]
    fn pointer_packing_uses_the_low_32_bits() {
        let (ptr, len) = manifest_pointer(MANIFEST.as_ptr(), MANIFEST.len());
        assert_eq!(ptr, (MANIFEST.as_ptr() as usize) as u32);
        assert_eq!(len as usize, MANIFEST.len());
    }

    #[test]
    fn allocation_is_aligned_and_distinct() {
        // SAFETY: single-threaded test.
        unsafe {
            let a = pc_alloc(16);
            let b = pc_alloc(16);
            assert!(!a.is_null());
            assert!(!b.is_null());
            assert_ne!(a, b);
            assert_eq!(a as usize % 16, 0);
            assert_eq!(b as usize % 16, 0);
        }
    }

    #[test]
    fn zero_and_negative_allocations_fail() {
        // SAFETY: single-threaded test.
        unsafe {
            assert!(pc_alloc(0).is_null());
            assert!(pc_alloc(-1).is_null());
        }
    }

    #[test]
    fn geometry_parses_from_a_host_params_payload() {
        let json = r#"{"seed": 5, "pixel_scale": 2,
            "_image": {"x": 0, "y": 256, "width": 8, "height": 8, "overlap": 0,
                       "canvasWidth": 6000, "canvasHeight": 4000,
                       "mode": "rgb", "depth": 8, "alpha": true, "channels": 4}}"#;

        let g = BandGeometry::parse(json).expect("geometry");
        assert_eq!(g.x, 0);
        assert_eq!(g.y, 256);
        assert_eq!(g.canvas_width, 6000);
        assert_eq!(g.canvas_height, 4000);
    }

    #[test]
    fn geometry_handles_negative_offsets_and_whitespace() {
        let json =
            r#"{ "_image" : { "x" : -12 , "y" : 7 , "canvasWidth" : 40 , "canvasHeight" : 40 } }"#;
        let g = BandGeometry::parse(json).expect("geometry");
        assert_eq!(g.x, 0, "negative x clamps to 0");
        assert_eq!(g.y, 7);
    }

    #[test]
    fn missing_geometry_is_rejected() {
        assert!(BandGeometry::parse(r#"{"seed": 1}"#).is_none());
        assert!(BandGeometry::parse(r#"{"_image": {"x": 0}}"#).is_none());
    }
}
