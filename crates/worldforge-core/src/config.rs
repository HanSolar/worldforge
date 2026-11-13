//! The generator's parameter set and its validation.
//!
//! Continent and island controls are intentionally separate so seed / continent
//! knobs can reshape large landmasses without the island controls dragging
//! continents around (and vice versa).

use serde::{Deserialize, Serialize};

use crate::seed::{Seed, SplitMix64};

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
pub const CONTINENT_VARIATION: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 55,
};
pub const CONTINENT_DRIFT: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 65,
};
pub const ISLAND_VARIATION: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 40,
};
pub const ISLAND_DENSITY: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 30,
};
pub const COAST_ROUGHNESS: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 45,
};
pub const MOUNTAIN_DENSITY: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 55,
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

/// Legacy single knob used by older manifests / CLI flags.
///
/// When present without the new continent/island fields, it is mapped into
/// `continent_variation`, `continent_drift`, and `island_variation`.
pub const LEGACY_VARIATION: Bounds = Bounds {
    min: 0,
    max: 100,
    default: 35,
};

/// Largest side the generator will accept, in document pixels.
pub const MAX_DIMENSION: u32 = 32_768;

/// Everything needed to reproduce a map.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldConfig {
    /// Reproduces a map.
    pub seed: u64,
    /// How strongly seed-driven noise reshapes continent silhouettes, 0..=100.
    pub continent_variation: i32,
    /// How far major continents may drift from the Earth-like bias anchors, 0..=100.
    pub continent_drift: i32,
    /// Seeded irregularity of island placement/shape, independent of continents.
    pub island_variation: i32,
    /// Frequency of small islands, 0..=100.
    pub island_density: i32,
    /// Coastline irregularity, 0..=100.
    pub coast_roughness: i32,
    /// Mountain coverage/intensity for procedural world mode, 0..=100.
    pub mountain_density: i32,
    /// Approximate land share, in percent.
    pub land_coverage: i32,
    /// Pixel-art sampling scale, in document pixels per world texel.
    pub pixel_scale: i32,
    /// Draft mode: clamp effective sampling to ~256px across for fast preview.
    pub preview_thumbnail_256: bool,
    /// Forces full-resolution rendering even when preview mode is default-on.
    pub generate_high_res: bool,
    /// Internal/CLI flag only — not a PhotoCraft dialog control.
    /// A bool checkbox cannot act as a Randomize button.
    pub randomize_settings: bool,
    /// Bright coastline treatment.
    pub coast_glow: bool,
    /// Palette id.
    pub palette: PaletteId,
    /// Generation style family.
    pub generator_type: GeneratorType,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            seed: SEED.default as u64,
            continent_variation: CONTINENT_VARIATION.default,
            continent_drift: CONTINENT_DRIFT.default,
            island_variation: ISLAND_VARIATION.default,
            island_density: ISLAND_DENSITY.default,
            coast_roughness: COAST_ROUGHNESS.default,
            mountain_density: MOUNTAIN_DENSITY.default,
            land_coverage: LAND_COVERAGE.default,
            pixel_scale: PIXEL_SCALE.default,
            preview_thumbnail_256: true,
            generate_high_res: false,
            randomize_settings: false,
            coast_glow: true,
            palette: PaletteId::ArcadeBlueGold,
            generator_type: GeneratorType::RandomWorld,
        }
    }
}

impl WorldConfig {
    /// The seed as a typed [`Seed`].
    pub fn seed(&self) -> Seed {
        Seed::new(self.seed)
    }

    /// Applies the legacy single `variation` knob when the new split fields are
    /// not being set explicitly by a caller.
    pub fn apply_legacy_variation(mut self, variation: i32) -> Self {
        let v = clamp_bounds(variation, LEGACY_VARIATION);
        self.continent_variation = v;
        self.continent_drift = v;
        // Islands get a quieter share of the old knob so legacy maps do not
        // suddenly fill with islands when only `variation` was set.
        self.island_variation = (v * 2 / 3).clamp(ISLAND_VARIATION.min, ISLAND_VARIATION.max);
        self
    }

    /// Clamps every field into range.
    pub fn clamped(mut self) -> Self {
        self.continent_variation = clamp_bounds(self.continent_variation, CONTINENT_VARIATION);
        self.continent_drift = clamp_bounds(self.continent_drift, CONTINENT_DRIFT);
        self.island_variation = clamp_bounds(self.island_variation, ISLAND_VARIATION);
        self.island_density = clamp_bounds(self.island_density, ISLAND_DENSITY);
        self.coast_roughness = clamp_bounds(self.coast_roughness, COAST_ROUGHNESS);
        self.mountain_density = clamp_bounds(self.mountain_density, MOUNTAIN_DENSITY);
        self.land_coverage = clamp_bounds(self.land_coverage, LAND_COVERAGE);
        self.pixel_scale = clamp_bounds(self.pixel_scale, PIXEL_SCALE);
        self
    }

    /// Rejects configurations the generator cannot honour.
    pub fn validate(&self) -> Result<(), ConfigError> {
        check(
            self.continent_variation,
            CONTINENT_VARIATION,
            "continent_variation",
        )?;
        check(self.continent_drift, CONTINENT_DRIFT, "continent_drift")?;
        check(self.island_variation, ISLAND_VARIATION, "island_variation")?;
        check(self.island_density, ISLAND_DENSITY, "island_density")?;
        check(self.coast_roughness, COAST_ROUGHNESS, "coast_roughness")?;
        check(self.mountain_density, MOUNTAIN_DENSITY, "mountain_density")?;
        check(self.land_coverage, LAND_COVERAGE, "land_coverage")?;
        check(self.pixel_scale, PIXEL_SCALE, "pixel_scale")?;
        Ok(())
    }

    /// Land threshold used by the classifier, derived from `land_coverage`.
    pub fn land_threshold(&self) -> f32 {
        let t = self.land_coverage as f32 / 100.0;
        (0.70 - t * 0.40).clamp(0.0, 1.0)
    }

    /// Continent silhouette variation as `0.0..=1.0`.
    pub fn continent_variation_weight(&self) -> f32 {
        self.continent_variation as f32 / 100.0
    }

    /// Continent placement drift as `0.0..=1.0`.
    pub fn continent_drift_weight(&self) -> f32 {
        self.continent_drift as f32 / 100.0
    }

    /// Island shape/placement variation as `0.0..=1.0`.
    pub fn island_variation_weight(&self) -> f32 {
        self.island_variation as f32 / 100.0
    }

    /// Roughness as a `0.0..=1.0` weight for coastline detail.
    pub fn roughness_weight(&self) -> f32 {
        self.coast_roughness as f32 / 100.0
    }

    /// Island density as a `0.0..=1.0` weight.
    pub fn island_weight(&self) -> f32 {
        self.island_density as f32 / 100.0
    }

    /// Mountain density as a `0.0..=1.0` weight (Procedural World Feature 1).
    pub fn mountain_weight(&self) -> f32 {
        self.mountain_density as f32 / 100.0
    }

    /// Returns a config randomized from the current seed.
    ///
    /// The same input seed produces the same randomized settings, which keeps
    /// banded wasm runs deterministic while still offering a one-click shuffle.
    pub fn randomized(mut self) -> Self {
        let mut rng = self.seed().rng("settings-randomizer");
        self.seed = bounded_u64(&mut rng, SEED);
        self.continent_variation = bounded_i32(&mut rng, CONTINENT_VARIATION);
        self.continent_drift = bounded_i32(&mut rng, CONTINENT_DRIFT);
        self.island_variation = bounded_i32(&mut rng, ISLAND_VARIATION);
        self.island_density = bounded_i32(&mut rng, ISLAND_DENSITY);
        self.coast_roughness = bounded_i32(&mut rng, COAST_ROUGHNESS);
        self.mountain_density = bounded_i32(&mut rng, MOUNTAIN_DENSITY);
        self.land_coverage = bounded_i32(&mut rng, LAND_COVERAGE);
        self.pixel_scale = bounded_i32(&mut rng, PIXEL_SCALE);
        self.coast_glow = (rng.next_u32() & 1) == 0;
        // Keep palette and generator_type as chosen by the user; only shuffle
        // numeric/bool map knobs so style and algorithm stay under control.
        self
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

fn bounded_i32(rng: &mut SplitMix64, bounds: Bounds) -> i32 {
    let span = (bounds.max - bounds.min + 1) as u32;
    bounds.min + (rng.next_u32() % span) as i32
}

fn bounded_u64(rng: &mut SplitMix64, bounds: Bounds) -> u64 {
    let max = bounds.max as u64;
    rng.next_u64() % (max + 1)
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

    #[error("unknown generator_type `{0}`")]
    UnknownGeneratorType(String),
}

/// Palette ids — color style only, reusable by every generator type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteId {
    /// Navy ocean, gold land, cyan coast — blue/gold arcade color style.
    #[default]
    ArcadeBlueGold,
}

/// Top-level generator family. Independent from [`PaletteId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratorType {
    /// Current random world generator (renamed from earth_arcade).
    #[default]
    RandomWorld,
    /// Procedural world tuning path (Feature 1+: mountains, later biomes).
    ProceduralWorld,
}

impl GeneratorType {
    pub const ALL: [GeneratorType; 2] =
        [GeneratorType::RandomWorld, GeneratorType::ProceduralWorld];

    pub const fn as_str(self) -> &'static str {
        match self {
            GeneratorType::RandomWorld => "random_world",
            GeneratorType::ProceduralWorld => "procedural_world",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        // Accept legacy earth_arcade as random_world so older params keep working.
        if value == "earth_arcade" {
            return Some(GeneratorType::RandomWorld);
        }
        GeneratorType::ALL.into_iter().find(|k| k.as_str() == value)
    }
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
            c.continent_variation = -50;
            c.continent_drift = 9_999;
            c.island_variation = -1;
            c.island_density = 9_999;
            c.mountain_density = 9_999;
            c.pixel_scale = 0;
        });
        let fixed = config.clamped();
        assert!(fixed.validate().is_ok());
        assert_eq!(fixed.continent_variation, CONTINENT_VARIATION.min);
        assert_eq!(fixed.continent_drift, CONTINENT_DRIFT.max);
        assert_eq!(fixed.island_variation, ISLAND_VARIATION.min);
        assert_eq!(fixed.island_density, ISLAND_DENSITY.max);
        assert_eq!(fixed.mountain_density, MOUNTAIN_DENSITY.max);
        assert_eq!(fixed.pixel_scale, PIXEL_SCALE.min);
    }

    #[test]
    fn higher_coverage_means_a_lower_threshold() {
        let low = config_with(|c| c.land_coverage = 10);
        let high = config_with(|c| c.land_coverage = 80);
        assert!(low.land_threshold() > high.land_threshold());
    }

    #[test]
    fn legacy_variation_maps_into_split_controls() {
        let mapped = WorldConfig::default().apply_legacy_variation(80);
        assert_eq!(mapped.continent_variation, 80);
        assert_eq!(mapped.continent_drift, 80);
        assert_eq!(mapped.island_variation, 53);
    }

    #[test]
    fn palette_round_trips() {
        for palette in PaletteId::ALL {
            assert_eq!(PaletteId::parse(palette.as_str()).unwrap(), palette);
        }
        assert!(PaletteId::parse("nope").is_err());
    }

    #[test]
    fn generator_type_round_trips() {
        for kind in GeneratorType::ALL {
            assert_eq!(GeneratorType::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(GeneratorType::parse("invalid"), None);
    }

    #[test]
    fn randomization_is_seed_deterministic_and_in_range() {
        let a = WorldConfig {
            seed: 42,
            ..WorldConfig::default()
        }
        .randomized();
        let b = WorldConfig {
            seed: 42,
            ..WorldConfig::default()
        }
        .randomized();
        assert_eq!(a, b);
        assert!(a.validate().is_ok());
    }
}
