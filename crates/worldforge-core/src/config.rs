//! The generator's parameter set and its validation.
//!
//! Spec §4.4: the dialog exposes a small set of controls, not every internal
//! algorithm constant. These mirror the manifest one-for-one.

use serde::{Deserialize, Serialize};

use crate::seed::Seed;

/// Inclusive parameter bounds.
///
/// The plugin manifest must publish exactly these numbers, and the core rejects
/// anything outside them, so a hostile or buggy host cannot put the generator
/// into a state the visual tests never covered.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub min: i32,
    pub max: i32,
    pub default: i32,
}

pub const SEED: Bounds = Bounds {
    min: 0,
    max: i32::MAX,
    default: 123_456,
};
pub const VARIATION: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 35,
};
pub const COAST_ROUGHNESS: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 45,
};
pub const ISLAND_DENSITY: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 30,
};
pub const LAND_COVERAGE: Bounds = Bounds {
    min: 5,
    max: 90,
    default: 38,
};
pub const PIXEL_SCALE: Bounds = Bounds {
    min: 1,
    max: 8,
    default: 2,
};

/// Largest side the generator will accept, in document pixels.
///
/// Banded execution caps how much work one call does, but a pathological
/// document still has to stay finite for `f32` coordinate math to stay exact.
pub const MAX_DIMENSION: u32 = 32_768;

/// Everything needed to reproduce a map.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldConfig {
    /// Reproduces a map.
    pub seed: u64,
    /// Procedural deviation from the Earth-like layout, 0..=100.
    pub variation: i32,
    /// Coastline irregularity, 0..=100.
    pub coast_roughness: i32,
    /// Frequency of small islands, 0..=100.
    pub island_density: i32,
    /// Approximate land share, in percent.
    pub land_coverage: i32,
    /// Pixel-art sampling scale, in document pixels per world texel.
    pub pixel_scale: i32,
    /// Bright coastline treatment.
    pub coast_glow: bool,
    /// Palette id.
    pub palette: PaletteId,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            seed: SEED.default as u64,
            variation: VARIATION.default,
            coast_roughness: COAST_ROUGHNESS.default,
            island_density: ISLAND_DENSITY.default,
            land_coverage: LAND_COVERAGE.default,
            pixel_scale: PIXEL_SCALE.default,
            coast_glow: true,
            palette: PaletteId::ArcadeBlueGold,
        }
    }
}

impl WorldConfig {
    /// The seed as a typed [`Seed`].
    pub fn seed(&self) -> Seed {
        Seed::new(self.seed)
    }

    /// Clamps every field into range.
    ///
    /// Clamping rather than rejecting is deliberate for numeric controls: the
    /// host already clamps, and a slightly out-of-range value should produce a
    /// usable map rather than an error dialog.
    pub fn clamped(mut self) -> Self {
        self.variation = clamp_bounds(self.variation, VARIATION);
        self.coast_roughness = clamp_bounds(self.coast_roughness, COAST_ROUGHNESS);
        self.island_density = clamp_bounds(self.island_density, ISLAND_DENSITY);
        self.land_coverage = clamp_bounds(self.land_coverage, LAND_COVERAGE);
        self.pixel_scale = clamp_bounds(self.pixel_scale, PIXEL_SCALE);
        self
    }

    /// Rejects configurations the generator cannot honour.
    pub fn validate(&self) -> Result<(), ConfigError> {
        check(self.variation, VARIATION, "variation")?;
        check(self.coast_roughness, COAST_ROUGHNESS, "coast_roughness")?;
        check(self.island_density, ISLAND_DENSITY, "island_density")?;
        check(self.land_coverage, LAND_COVERAGE, "land_coverage")?;
        check(self.pixel_scale, PIXEL_SCALE, "pixel_scale")?;
        Ok(())
    }

    /// Land threshold used by the classifier, derived from `land_coverage`.
    ///
    /// A higher requested coverage lowers the threshold, so more of the field
    /// counts as land.
    pub fn land_threshold(&self) -> f32 {
        // Calibrated so 0 % -> 0.70 and 100 % -> 0.30; the field's own
        // distribution is roughly centred on 0.5.
        let t = self.land_coverage as f32 / 100.0;
        (0.70 - t * 0.40).clamp(0.0, 1.0)
    }

    /// Variation as a `0.0..=1.0` weight for the continental bias override.
    pub fn variation_weight(&self) -> f32 {
        self.variation as f32 / 100.0
    }

    /// Roughness as a `0.0..=1.0` weight for coastline detail.
    pub fn roughness_weight(&self) -> f32 {
        self.coast_roughness as f32 / 100.0
    }

    /// Island density as a `0.0..=1.0` weight.
    pub fn island_weight(&self) -> f32 {
        self.island_density as f32 / 100.0
    }
}

fn clamp_bounds(value: i32, bounds: Bounds) -> i32 {
    value.clamp(bounds.min, bounds.max)
}

fn check(value: i32, bounds: Bounds, field: &'static str) -> Result<(), ConfigError> {
    if value < bounds.min || value > bounds.max {
        return Err(ConfigError::OutOfRange {
            field,
            value,
            min: bounds.min,
            max: bounds.max,
        });
    }
    Ok(())
}

/// Why a [`WorldConfig`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("{field} is {value}, expected {min}..={max}")]
    OutOfRange {
        field: &'static str,
        value: i32,
        min: i32,
        max: i32,
    },

    #[error("unknown palette `{0}`")]
    UnknownPalette(String),
}

/// Palette ids the generator understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteId {
    /// Navy ocean, gold land, cyan coast — the M1 look.
    #[default]
    ArcadeBlueGold,
}

impl PaletteId {
    /// Every palette, in manifest order.
    pub const ALL: [PaletteId; 1] = [PaletteId::ArcadeBlueGold];

    /// The manifest `choice` string.
    pub const fn as_str(self) -> &'static str {
        match self {
            PaletteId::ArcadeBlueGold => "arcade_blue_gold",
        }
    }

    /// Parses a manifest `choice` string.
    pub fn parse(value: &str) -> Result<Self, ConfigError> {
        PaletteId::ALL
            .into_iter()
            .find(|p| p.as_str() == value)
            .ok_or_else(|| ConfigError::UnknownPalette(value.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(f: impl FnOnce(&mut WorldConfig)) -> WorldConfig {
        let mut config = WorldConfig::default();
        f(&mut config);
        config
    }

    #[test]
    fn default_is_valid() {
        assert!(WorldConfig::default().validate().is_ok());
    }

    #[test]
    fn out_of_range_is_rejected() {
        let config = config_with(|c| c.land_coverage = 99);
        assert!(matches!(
            config.validate(),
            Err(ConfigError::OutOfRange {
                field: "land_coverage",
                ..
            })
        ));
    }

    #[test]
    fn clamping_always_produces_a_valid_config() {
        let config = config_with(|c| {
            c.variation = -50;
            c.island_density = 9_999;
            c.pixel_scale = 0;
        });
        let fixed = config.clamped();
        assert!(fixed.validate().is_ok());
        assert_eq!(fixed.variation, VARIATION.min);
        assert_eq!(fixed.island_density, ISLAND_DENSITY.max);
        assert_eq!(fixed.pixel_scale, PIXEL_SCALE.min);
    }

    #[test]
    fn higher_coverage_means_a_lower_threshold() {
        let low = config_with(|c| c.land_coverage = 10);
        let high = config_with(|c| c.land_coverage = 80);
        assert!(low.land_threshold() > high.land_threshold());
    }

    #[test]
    fn palette_round_trips() {
        for palette in PaletteId::ALL {
            assert_eq!(PaletteId::parse(palette.as_str()).unwrap(), palette);
        }
        assert!(PaletteId::parse("nope").is_err());
    }
}
