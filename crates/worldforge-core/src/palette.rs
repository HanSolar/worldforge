//! Colours, all of them, in one place (spec §4.5).
//!
//! "Keep the palette in one module so it can be tuned without changing the
//! generator." Nothing else in the engine hard-codes a colour.

use crate::config::PaletteId;

/// A colour in `f32` linear-as-authored sRGB components.
///
/// Components are `0.0..=1.0`, matching the samples the host expects, so a
/// palette colour goes straight into a pixel buffer with no scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Colour {
    /// Builds from 8-bit sRGB, the natural way to read spec hex values.
    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xFF) as f32 / 255.0,
            g: ((rgb >> 8) & 0xFF) as f32 / 255.0,
            b: (rgb & 0xFF) as f32 / 255.0,
        }
    }

    /// Linear blend; `t` is clamped.
    pub fn lerp(self, other: Colour, t: f32) -> Colour {
        if !t.is_finite() {
            return self;
        }
        let t = t.clamp(0.0, 1.0);
        Colour {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }

    /// Scales brightness, clamped to the displayable range.
    pub fn scaled(self, factor: f32) -> Colour {
        let f = if factor.is_finite() { factor } else { 1.0 };
        Colour {
            r: (self.r * f).clamp(0.0, 1.0),
            g: (self.g * f).clamp(0.0, 1.0),
            b: (self.b * f).clamp(0.0, 1.0),
        }
    }
}

/// One named palette: every colour the renderer may use.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub deep_ocean: Colour,
    pub ocean_highlight: Colour,
    pub land_base: Colour,
    pub land_highlight: Colour,
    pub coastline: Colour,
    pub coast_glow: Colour,
}

impl Palette {
    /// Navy ocean, gold land, cyan shore — the M1 arcade look.
    ///
    /// Spec §4.5 lists the same seven roles; the seventh (the glow) is folded
    /// into [`Palette::coast_glow`] as a falloff rather than a flat colour.
    pub const fn arcade_blue_gold() -> Self {
        Self {
            deep_ocean: Colour::hex(0x071A72),
            ocean_highlight: Colour::hex(0x123DAA),
            land_base: Colour::hex(0xC99A5A),
            land_highlight: Colour::hex(0xE0B879),
            coastline: Colour::hex(0x35D8FF),
            coast_glow: Colour::hex(0x087DDB),
        }
    }

    /// Looks up a palette by id.
    pub const fn get(id: PaletteId) -> Self {
        match id {
            PaletteId::ArcadeBlueGold => Palette::arcade_blue_gold(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_conversion_matches_the_spec() {
        let p = Palette::arcade_blue_gold();
        let deep = p.deep_ocean;
        assert!((deep.r - 7.0 / 255.0).abs() < 1e-6);
        assert!((deep.g - 26.0 / 255.0).abs() < 1e-6);
        assert!((deep.b - 114.0 / 255.0).abs() < 1e-6);

        let coast = p.coastline;
        assert!((coast.b - 1.0).abs() < 1e-6, "coastline blue should be FF");
    }

    #[test]
    fn lerp_hits_both_ends() {
        let a = Colour::hex(0x000000);
        let b = Colour::hex(0xFFFFFF);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(a.lerp(b, -5.0), a, "t must clamp");
        assert_eq!(a.lerp(b, 5.0), b, "t must clamp");
    }

    #[test]
    fn lerp_survives_nan() {
        let a = Colour::hex(0x112233);
        let b = Colour::hex(0xFFFFFF);
        assert_eq!(a.lerp(b, f32::NAN), a);
    }

    #[test]
    fn scaled_stays_in_range() {
        let c = Colour::hex(0xFFFFFF);
        let s = c.scaled(4.0);
        assert!(s.r <= 1.0 && s.g <= 1.0 && s.b <= 1.0);
    }

    #[test]
    fn every_palette_id_resolves() {
        for id in PaletteId::ALL {
            let p = Palette::get(id);
            assert!(
                p.coastline.b > 0.9,
                "{} coast should be bright",
                id.as_str()
            );
        }
    }
}
