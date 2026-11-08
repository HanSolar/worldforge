//! WorldForge core — host-independent procedural world generation.
//!
//! This crate knows nothing about PhotoCraft, WebAssembly, SVG, or chess
//! (spec §3.2). It generates a deterministic Earth-like land/ocean field and
//! renders regions of it as pixels.
//!
//! # Why there is no image type
//!
//! The first host runs the plugin in band-sized WebAssembly instances, each in a
//! fresh instance with no surviving state (spec §5.3). So the public surface is
//! coordinate-in: [`landmass::Field`] answers questions about one point,
//! [`raster::render_band`] fills one rectangle. Any caller holding a whole image
//! can just ask for one band the size of the image.
//!
//! # Determinism
//!
//! Same config, same seed, same dimensions ⇒ same bytes, on every platform.
//! Randomness comes only from [`seed`], and each stage draws from its own
//! stream so tuning one does not shift another.

pub mod coastline;
pub mod config;
pub mod landmass;
pub mod noise;
pub mod palette;
pub mod raster;
pub mod seed;

pub use config::{ConfigError, PaletteId, WorldConfig};
pub use landmass::Field;
pub use palette::{Colour, Palette};
pub use raster::{Band, RenderOptions, render_band};
pub use seed::{GENERATOR_VERSION, Seed};

/// Errors from configuration parsing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid configuration: {0}")]
    Config(#[from] ConfigError),

    #[error("malformed parameters: {0}")]
    Params(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Parses the host's parameter JSON into a [`WorldConfig`].
///
/// Unknown keys are ignored rather than rejected: the host appends `_image` and
/// may add fields in future ABI revisions. Out-of-range numbers are clamped.
///
/// Legacy `variation` is honoured when the new continent/island knobs are
/// absent, so older PhotoCraft dialog saves keep working.
pub fn config_from_params(json: &str) -> Result<WorldConfig> {
    #[derive(serde::Deserialize)]
    struct Params {
        #[serde(default)]
        seed: Option<u64>,
        #[serde(default)]
        variation: Option<i32>,
        #[serde(default)]
        continent_variation: Option<i32>,
        #[serde(default)]
        continent_drift: Option<i32>,
        #[serde(default)]
        island_variation: Option<i32>,
        #[serde(default)]
        coast_roughness: Option<i32>,
        #[serde(default)]
        island_density: Option<i32>,
        #[serde(default)]
        land_coverage: Option<i32>,
        #[serde(default)]
        pixel_scale: Option<i32>,
        #[serde(default)]
        preview_thumbnail_256: Option<bool>,
        #[serde(default)]
        coast_glow: Option<bool>,
        #[serde(default)]
        palette: Option<String>,
    }

    let parsed: Params = serde_json::from_str(json)?;
    let mut config = WorldConfig::default();

    if let Some(seed) = parsed.seed {
        config.seed = seed;
    }

    let has_split = parsed.continent_variation.is_some()
        || parsed.continent_drift.is_some()
        || parsed.island_variation.is_some();

    if !has_split {
        if let Some(v) = parsed.variation {
            config = config.apply_legacy_variation(v);
        }
    }

    if let Some(v) = parsed.continent_variation {
        config.continent_variation = v;
    }
    if let Some(v) = parsed.continent_drift {
        config.continent_drift = v;
    }
    if let Some(v) = parsed.island_variation {
        config.island_variation = v;
    }
    if let Some(v) = parsed.coast_roughness {
        config.coast_roughness = v;
    }
    if let Some(v) = parsed.island_density {
        config.island_density = v;
    }
    if let Some(v) = parsed.land_coverage {
        config.land_coverage = v;
    }
    if let Some(v) = parsed.pixel_scale {
        config.pixel_scale = v;
    }
    if let Some(v) = parsed.preview_thumbnail_256 {
        config.preview_thumbnail_256 = v;
    }
    if let Some(v) = parsed.coast_glow {
        config.coast_glow = v;
    }
    if let Some(palette) = parsed.palette.as_deref() {
        config.palette = PaletteId::parse(palette)?;
    }

    Ok(config.clamped())
}

/// Convenience: generate a whole world as one band.
pub fn render_image(config: WorldConfig, width: u32, height: u32, channels: u32) -> Vec<f32> {
    let field = Field::new(config);
    let band = Band::new(0, 0, width, height, channels);
    let options = RenderOptions {
        canvas_width: width,
        canvas_height: height,
        coast_glow: config.coast_glow,
        pixel_scale: config.pixel_scale,
        preview_thumbnail_256: config.preview_thumbnail_256,
    };

    let mut buffer = vec![0.0f32; band.sample_count()];
    render_band(&field, &band, &options, &mut buffer);
    buffer
}

/// Fraction of the world that is land, by sampling the field.
pub fn land_fraction(config: WorldConfig, samples_per_axis: u32) -> f32 {
    let field = Field::new(config);
    let n = samples_per_axis.max(1);
    let mut land = 0u32;

    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32;
            let v = (y as f32 + 0.5) / n as f32;
            if field.is_land(u, v) {
                land += 1;
            }
        }
    }

    land as f32 / (n * n) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_variation_param_maps_when_split_absent() {
        let config = config_from_params(r#"{"seed":1,"variation":70}"#).unwrap();
        assert_eq!(config.continent_variation, 70);
        assert_eq!(config.continent_drift, 70);
        assert_eq!(config.island_variation, 46);
    }

    #[test]
    fn explicit_split_params_win_over_legacy_variation() {
        let config = config_from_params(
            r#"{"seed":1,"variation":10,"continent_variation":80,"continent_drift":20,"island_variation":5}"#,
        )
        .unwrap();
        assert_eq!(config.continent_variation, 80);
        assert_eq!(config.continent_drift, 20);
        assert_eq!(config.island_variation, 5);
    }

    #[test]
    fn preview_thumbnail_flag_parses() {
        let off = config_from_params(r#"{"preview_thumbnail_256":false}"#).unwrap();
        let on = config_from_params(r#"{"preview_thumbnail_256":true}"#).unwrap();
        assert!(!off.preview_thumbnail_256);
        assert!(on.preview_thumbnail_256);
    }
}
