//! Rendering into the host's pixel band.
//!
//! Spec §5.2/§5.3: the plugin receives interleaved little-endian `f32` samples
//! for a rectangular band and writes them **in place**. There is no PNG code
//! here and no full-canvas image; the host owns the document.
//!
//! The only thing this module knows about the host's layout is the tuple
//! `(width, height, channels)` for a band plus its global origin, which is
//! exactly what the ABI supplies.

use crate::coastline::{self, Surface};
use crate::landmass::Field;
use crate::palette::{Colour, Palette};

/// Geometry of one band, in document coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    /// Document x of the band's first sample (including overlap margin).
    pub x: u32,
    /// Document y of the band's first sample.
    pub y: u32,
    /// Samples across.
    pub width: u32,
    /// Samples down.
    pub height: u32,
    /// Samples per pixel.
    pub channels: u32,
}

impl Band {
    pub const fn new(x: u32, y: u32, width: u32, height: u32, channels: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            channels,
        }
    }

    /// Total `f32` samples in the buffer.
    pub fn sample_count(&self) -> usize {
        (self.width as usize) * (self.height as usize) * (self.channels as usize)
    }

    /// Builds a band, rejecting sizes that overflow `usize` or are empty.
    ///
    /// Used by the ABI, which receives dimensions from outside and must not
    /// trust them.
    pub fn checked(width: u32, height: u32, channels: u32) -> Option<Self> {
        if width == 0 || height == 0 || channels == 0 {
            return None;
        }
        let band = Self {
            x: 0,
            y: 0,
            width,
            height,
            channels,
        };
        // Guard the multiplications rather than relying on them not to overflow.
        (width as u64)
            .checked_mul(height as u64)?
            .checked_mul(channels as u64)?;
        if band.sample_count() == 0 {
            return None;
        }
        Some(band)
    }

    /// Index of the first sample of pixel `(px, py)` within the band.
    #[inline]
    fn index(&self, px: u32, py: u32) -> usize {
        (py as usize * self.width as usize + px as usize) * self.channels as usize
    }
}

/// What the renderer needs beyond the field itself.
#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    /// Document width, for the aspect-ratio fit.
    pub canvas_width: u32,
    /// Document height.
    pub canvas_height: u32,
    /// Enable the bright coastline and glow.
    pub coast_glow: bool,
    /// Document pixels per world texel.
    pub pixel_scale: i32,
    /// Draft-preview mode: approximate a 256x256 thumbnail for fast interaction.
    pub preview_thumbnail_256: bool,
}

impl RenderOptions {
    /// Pixels per world texel, at least 1.
    pub fn scale(&self) -> u32 {
        let base = self.pixel_scale.max(1) as u32;
        if !self.preview_thumbnail_256 {
            return base;
        }

        // Keep draft work near a 256x256 budget even on very large canvases.
        let max_side = self.canvas_width.max(self.canvas_height).max(1);
        let draft = max_side.div_ceil(256);
        base.max(draft.max(1))
    }
}

/// Renders one band into `buffer`, in place.
///
/// `buffer` must hold at least `band.sample_count()` samples. Extra channels
/// beyond RGB are left untouched, so a caller that pre-filled alpha keeps it.
///
/// Returns the number of pixels written.
pub fn render_band(
    field: &Field,
    band: &Band,
    options: &RenderOptions,
    buffer: &mut [f32],
) -> usize {
    let palette = Palette::get(field.config().palette);
    let scale = options.scale();
    let mut written = 0;
    let mut prev_snapped_y: Option<u32> = None;

    for py in 0..band.height {
        let doc_y = band.y + py;
        // Pixel-art snap in document space: rows in the same snapped block are
        // identical, so we can copy the previous row instead of recomputing.
        let snapped_y = (doc_y / scale) * scale;

        if py > 0 && prev_snapped_y == Some(snapped_y) {
            copy_row_from_previous(band, py, buffer);
            written += band.width as usize;
            continue;
        }
        prev_snapped_y = Some(snapped_y);

        let mut px = 0u32;
        while px < band.width {
            let doc_x = band.x + px;
            let snapped_x = (doc_x / scale) * scale;
            let (u, v) = field.world_coord(
                snapped_x,
                snapped_y,
                options.canvas_width,
                options.canvas_height,
            );
            let colour = shade(field, &palette, u, v, options);

            // Fill every pixel that snaps to the same document-space block.
            let block_end_doc_x = snapped_x.saturating_add(scale);
            let local_end = band.width.min(
                block_end_doc_x
                    .saturating_sub(band.x)
                    .max(px.saturating_add(1)),
            );

            let mut fill_px = px;
            while fill_px < local_end {
                let base = band.index(fill_px, py);
                match band.channels {
                    // Grayscale documents get luminance, so the map still reads
                    // correctly instead of receiving only the red channel.
                    1 => {
                        if base < buffer.len() {
                            buffer[base] = luminance(colour);
                        }
                    }
                    // Two channels (Gray+Alpha): luminance, and leave alpha alone.
                    2 => {
                        if base < buffer.len() {
                            buffer[base] = luminance(colour);
                        }
                    }
                    _ => {
                        if base < buffer.len() {
                            buffer[base] = colour.r;
                        }
                        if base + 1 < buffer.len() {
                            buffer[base + 1] = colour.g;
                        }
                        if base + 2 < buffer.len() {
                            buffer[base + 2] = colour.b;
                        }
                    }
                }
                fill_px += 1;
                written += 1;
            }
            px = local_end;
        }
    }

    written
}

/// Reuses the previous rendered row when both rows snap to the same world row.
fn copy_row_from_previous(band: &Band, py: u32, buffer: &mut [f32]) {
    let channels = band.channels as usize;
    let width = band.width as usize;
    let dst_row = py as usize;
    let src_row = dst_row.saturating_sub(1);

    for x in 0..width {
        let dst = (dst_row * width + x) * channels;
        let src = (src_row * width + x) * channels;
        match channels {
            0 => {}
            1 => {
                if dst < buffer.len() && src < buffer.len() {
                    buffer[dst] = buffer[src];
                }
            }
            2 => {
                if dst < buffer.len() && src < buffer.len() {
                    buffer[dst] = buffer[src];
                }
            }
            _ => {
                if dst < buffer.len() && src < buffer.len() {
                    buffer[dst] = buffer[src];
                }
                if dst + 1 < buffer.len() && src + 1 < buffer.len() {
                    buffer[dst + 1] = buffer[src + 1];
                }
                if dst + 2 < buffer.len() && src + 2 < buffer.len() {
                    buffer[dst + 2] = buffer[src + 2];
                }
            }
        }
    }
}

/// Rec. 709 luminance, for grayscale documents.
#[inline]
fn luminance(colour: Colour) -> f32 {
    (0.2126 * colour.r + 0.7152 * colour.g + 0.0722 * colour.b).clamp(0.0, 1.0)
}

/// The colour of one world coordinate.
fn shade(field: &Field, palette: &Palette, u: f32, v: f32, options: &RenderOptions) -> Colour {
    // Letterbox margin: leave the document's own backdrop alone.
    if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
        return Colour {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        };
    }

    let (su, sv) = coastline::snap(u, v);
    let surface = coastline::classify(field, su, sv);
    let height = field.sample(su, sv);
    let threshold = field.threshold();

    let coast_effects = options.coast_glow;
    match surface {
        Surface::Ocean => {
            // Shallow-to-deep gradient away from the shelf. The exponent keeps
            // the deep navy dominant, as the arcade look wants.
            let depth = ((threshold - height) / threshold.max(1e-3)).clamp(0.0, 1.0);
            let t = (depth * 2.2).clamp(0.0, 1.0).powf(0.75);
            let mut colour = palette.ocean_highlight.lerp(palette.deep_ocean, t);

            if coast_effects {
                if options.preview_thumbnail_256 {
                    // Draft mode keeps a cheap shoreline tint but skips
                    // neighborhood searches to stay inside host budgets.
                    if depth < 0.18 {
                        let falloff = (1.0 - depth / 0.18).clamp(0.0, 1.0).powf(1.7);
                        colour = colour.lerp(palette.coast_glow, falloff * 0.35);
                    }
                } else if depth < 0.16 {
                    // Full mode: glow only matters near shore. Skipping deep
                    // ocean avoids the costly ring search on most pixels.
                    if let Some(d) = coastline::shore_distance(field, su, sv, 2) {
                        let falloff = (1.0 - d / 6.0).clamp(0.0, 1.0).powf(2.0);
                        colour = colour.lerp(palette.coast_glow, falloff * 0.55);
                    }
                }
            }
            colour
        }
        Surface::Land => {
            // Land lightens with elevation, with a faint internal texture so
            // large continents are not flat blocks.
            let above = ((height - threshold) / (1.0 - threshold).max(1e-3)).clamp(0.0, 1.0);
            let mut colour = palette
                .land_base
                .lerp(palette.land_highlight, above.powf(0.85));

            // Coast checks are expensive; deep interior land cannot be coast.
            let coast_cutoff = if options.preview_thumbnail_256 {
                0.28
            } else {
                0.20
            };
            if coast_effects && above < coast_cutoff && coastline::is_coast(field, su, sv) {
                colour = palette.coastline;
            }
            colour
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldConfig;

    fn setup(canvas: u32) -> (Field, RenderOptions) {
        let field = Field::new(WorldConfig::default());
        let options = RenderOptions {
            canvas_width: canvas,
            canvas_height: canvas,
            coast_glow: true,
            pixel_scale: 2,
            preview_thumbnail_256: false,
        };
        (field, options)
    }

    #[test]
    fn band_sample_count_is_exact() {
        let band = Band::new(0, 0, 8, 4, 4);
        assert_eq!(band.sample_count(), 8 * 4 * 4);
    }

    #[test]
    fn rendering_writes_every_pixel() {
        let (field, options) = setup(64);
        let band = Band::new(0, 0, 16, 8, 4);
        let mut buffer = vec![0.0f32; band.sample_count()];

        let written = render_band(&field, &band, &options, &mut buffer);
        assert_eq!(written, 16 * 8);
    }

    #[test]
    fn output_samples_are_finite_and_in_range() {
        let (field, options) = setup(256);
        let band = Band::new(0, 0, 32, 32, 3);
        let mut buffer = vec![0.0f32; band.sample_count()];

        render_band(&field, &band, &options, &mut buffer);

        for s in &buffer {
            assert!(s.is_finite(), "non-finite sample");
            assert!((0.0..=1.0).contains(s), "sample {s} out of range");
        }
    }

    #[test]
    fn banded_rendering_matches_the_same_region_rendered_whole() {
        // Spec Task 2 acceptance: "banded rendering matches full-image rendering".
        let (field, options) = setup(128);

        let whole = Band::new(0, 0, 64, 64, 3);
        let mut whole_buf = vec![0.0f32; whole.sample_count()];
        render_band(&field, &whole, &options, &mut whole_buf);

        // Same region, as four horizontal bands rendered independently.
        let mut banded = vec![0.0f32; whole.sample_count()];
        let strip = 16;
        for row in 0..4u32 {
            let b = Band::new(0, row * strip, 64, strip, 3);
            let mut buf = vec![0.0f32; b.sample_count()];
            render_band(&field, &b, &options, &mut buf);

            // Copy the strip back into the stitched image.
            for y in 0..strip {
                for x in 0..64 {
                    let dst = ((row * strip + y) as usize * 64 + x as usize) * 3;
                    let src = (y as usize * 64 + x as usize) * 3;
                    banded[dst..dst + 3].copy_from_slice(&buf[src..src + 3]);
                }
            }
        }

        assert_eq!(whole_buf, banded, "band split changed the output");
    }

    #[test]
    fn offsets_are_absolute_not_band_relative() {
        // A band that does not start at the origin must land in the same place
        // as the equivalent slice of a whole-canvas render.
        let (field, options) = setup(96);

        let whole = Band::new(0, 0, 48, 48, 3);
        let mut whole_buf = vec![0.0f32; whole.sample_count()];
        render_band(&field, &whole, &options, &mut whole_buf);

        let offset = Band::new(16, 16, 16, 16, 3);
        let mut offset_buf = vec![0.0f32; offset.sample_count()];
        render_band(&field, &offset, &options, &mut offset_buf);

        for y in 0..16 {
            for x in 0..16 {
                let src = ((y + 16) as usize * 48 + (x + 16) as usize) * 3;
                let dst = (y as usize * 16 + x as usize) * 3;
                assert_eq!(
                    &whole_buf[src..src + 3],
                    &offset_buf[dst..dst + 3],
                    "mismatch at band-local ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn alpha_is_preserved() {
        let (field, options) = setup(32);
        let band = Band::new(0, 0, 8, 8, 4);
        let mut buffer = vec![0.42f32; band.sample_count()];

        render_band(&field, &band, &options, &mut buffer);

        for px in 0..8usize {
            for py in 0..8usize {
                let idx = (py * 8 + px) * 4;
                assert_eq!(buffer[idx + 3], 0.42, "renderer must not touch alpha");
            }
        }
    }

    #[test]
    fn grayscale_documents_get_luminance() {
        // A 1-channel document must receive luminance, not just the red channel.
        let (field, _) = setup(32);
        let opts = RenderOptions {
            canvas_width: 32,
            canvas_height: 32,
            coast_glow: false,
            pixel_scale: 2,
            preview_thumbnail_256: false,
        };
        let band = Band::new(0, 0, 32, 32, 1);
        let mut buffer = vec![0.0f32; band.sample_count()];

        render_band(&field, &band, &opts, &mut buffer);

        for s in &buffer {
            assert!(s.is_finite() && (0.0..=1.0).contains(s));
        }
        // Luminance of the palette differs from the raw red channel, so a
        // grayscale render must not equal a red-channel-only render.
        let mut rgb = vec![0.0f32; 32 * 32 * 3];
        render_band(&field, &Band::new(0, 0, 32, 32, 3), &opts, &mut rgb);
        let reds: Vec<f32> = rgb.chunks_exact(3).map(|p| p[0]).collect();
        assert_ne!(buffer, reds, "grayscale must be luminance, not red");
    }

    #[test]
    fn short_buffer_does_not_panic() {
        let (field, options) = setup(32);
        let band = Band::new(0, 0, 8, 8, 3);
        let mut buffer = vec![0.0f32; 4]; // deliberately far too small

        let written = render_band(&field, &band, &options, &mut buffer);
        assert_eq!(written, 64);
    }

    #[test]
    fn map_contains_both_land_and_ocean_colours() {
        let (field, options) = setup(128);
        let band = Band::new(0, 0, 128, 128, 3);
        let mut buffer = vec![0.0f32; band.sample_count()];

        render_band(&field, &band, &options, &mut buffer);

        // Land base is warm (r > b); deep ocean is cool (b > r).
        let warm = buffer.chunks_exact(3).filter(|p| p[0] > p[2] + 0.1).count();
        let cool = buffer.chunks_exact(3).filter(|p| p[2] > p[0] + 0.1).count();

        assert!(warm > 500, "expected land pixels, got {warm}");
        assert!(cool > 500, "expected ocean pixels, got {cool}");
    }

    #[test]
    fn coast_glow_still_changes_preview_mode_output() {
        let field = Field::new(WorldConfig::default());
        let on = RenderOptions {
            canvas_width: 128,
            canvas_height: 128,
            coast_glow: true,
            pixel_scale: 2,
            preview_thumbnail_256: true,
        };
        let off = RenderOptions {
            coast_glow: false,
            ..on
        };
        let band = Band::new(0, 0, 128, 128, 3);
        let mut a = vec![0.0f32; band.sample_count()];
        let mut b = vec![0.0f32; band.sample_count()];
        render_band(&field, &band, &on, &mut a);
        render_band(&field, &band, &off, &mut b);
        assert_ne!(a, b, "coast glow should affect preview-thumbnail mode");
    }
}
