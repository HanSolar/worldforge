//! The land/water field, evaluated **per coordinate** rather than as an image.
//!
//! Spec §5.3 and §5.4 make the shape of this module non-negotiable:
//!
//! - PhotoCraft splits a layer into bands and runs each band in a *fresh*
//!   WebAssembly instance. No state survives between calls.
//! - A full-canvas buffer inside a band is forbidden.
//!
//! Continent macro-layout and island generation use **separate seed streams**
//! and separate config knobs so changing the seed (or continent controls) can
//! move large landmasses without islands dragging continents around.

use crate::config::WorldConfig;
use crate::noise::{fbm, fbm_signed, value_noise};
use crate::seed::SplitMix64;

/// Continental-bias layout: where the generator wants large landmasses.
///
/// This is a low-resolution hand-authored control field, not noise. Spec §4.2:
/// "the initial Earth-like profile should use a designed continental-bias mask
/// or control field." Seed-driven transform + macro noise move that layout so
/// different seeds are not stuck on the same anchors.
const BIAS_W: usize = 16;
const BIAS_H: usize = 9;

#[rustfmt::skip]
const CONTINENT_BIAS: [f32; BIAS_W * BIAS_H] = [
    0.00, 0.00, 0.05, 0.10, 0.20, 0.45, 0.60, 0.55, 0.25, 0.10, 0.30, 0.65, 0.80, 0.70, 0.35, 0.05,
    0.00, 0.05, 0.15, 0.35, 0.65, 0.85, 0.90, 0.75, 0.30, 0.15, 0.55, 0.90, 0.95, 0.80, 0.45, 0.10,
    0.05, 0.20, 0.45, 0.70, 0.90, 0.95, 0.90, 0.65, 0.25, 0.20, 0.60, 0.95, 0.95, 0.85, 0.55, 0.15,
    0.10, 0.35, 0.60, 0.80, 0.90, 0.85, 0.70, 0.45, 0.20, 0.25, 0.50, 0.80, 0.90, 0.80, 0.50, 0.20,
    0.10, 0.30, 0.50, 0.65, 0.70, 0.60, 0.45, 0.30, 0.15, 0.20, 0.35, 0.55, 0.70, 0.60, 0.35, 0.15,
    0.05, 0.15, 0.25, 0.35, 0.40, 0.35, 0.30, 0.35, 0.45, 0.55, 0.65, 0.70, 0.65, 0.45, 0.25, 0.10,
    0.10, 0.20, 0.30, 0.40, 0.50, 0.55, 0.65, 0.75, 0.80, 0.85, 0.85, 0.80, 0.70, 0.50, 0.30, 0.15,
    0.15, 0.30, 0.50, 0.65, 0.75, 0.80, 0.85, 0.90, 0.90, 0.85, 0.75, 0.60, 0.45, 0.30, 0.20, 0.10,
    0.10, 0.20, 0.35, 0.50, 0.60, 0.65, 0.70, 0.70, 0.65, 0.55, 0.45, 0.35, 0.25, 0.15, 0.10, 0.05,
];

/// Bilinear sample of the continental bias at normalised `(u, v)`.
fn continental_bias(u: f32, v: f32) -> f32 {
    let x = u.rem_euclid(1.0) * (BIAS_W as f32 - 1.0);
    let y = v.clamp(0.0, 1.0) * (BIAS_H as f32 - 1.0);

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(BIAS_W - 1);
    let y1 = (y0 + 1).min(BIAS_H - 1);

    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let at = |cx: usize, cy: usize| CONTINENT_BIAS[cy * BIAS_W + cx];
    let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
    let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
    top + (bottom - top) * fy
}

fn latitude_falloff(v: f32) -> f32 {
    let t = ((v - 0.5) * 2.0).abs();
    let eased = t * t * t;
    1.0 - 0.45 * eased
}

/// Seed-derived transform applied to bias coordinates.
#[derive(Debug, Clone, Copy)]
struct ContinentTransform {
    /// East-west wrap offset in normalised units.
    shift_u: f32,
    /// North-south offset (clamped after apply).
    shift_v: f32,
    /// Small rotation in radians around the map centre.
    rotation: f32,
    /// How strongly the seed macro field replaces the hand bias.
    macro_mix: f32,
}

impl ContinentTransform {
    fn from_seed(seed: u64, drift: f32, variation: f32) -> Self {
        let mut rng = SplitMix64::new(seed);
        // Drift controls placement; variation controls reshape mix.
        let drift = drift.clamp(0.0, 1.0);
        let variation = variation.clamp(0.0, 1.0);
        Self {
            shift_u: (rng.next_f32() - 0.5) * 2.0 * (0.08 + drift * 0.55),
            shift_v: (rng.next_f32() - 0.5) * 2.0 * (0.04 + drift * 0.28),
            rotation: (rng.next_f32() - 0.5) * 2.0 * (0.08 + drift * 0.55),
            macro_mix: 0.18 + variation * 0.62,
        }
    }

    fn map(&self, u: f32, v: f32) -> (f32, f32) {
        // Rotate around the centre, then translate. East-west wraps; latitude
        // stays in 0..=1 so poles do not fold.
        let cu = u - 0.5;
        let cv = v - 0.5;
        let (s, c) = self.rotation.sin_cos();
        let ru = cu * c - cv * s;
        let rv = cu * s + cv * c;
        let ou = (ru + 0.5 + self.shift_u).rem_euclid(1.0);
        let ov = (rv + 0.5 + self.shift_v).clamp(0.0, 1.0);
        (ou, ov)
    }
}

/// A world field bound to one configuration.
#[derive(Debug, Clone, Copy)]
pub struct Field {
    config: WorldConfig,
    seed_warp: u32,
    seed_detail: u32,
    seed_macro: u32,
    seed_island: u32,
    seed_island_warp: u32,
    seed_coast: u32,
    transform: ContinentTransform,
    threshold: f32,
}

impl Field {
    /// Prepares the field for `config`.
    pub fn new(config: WorldConfig) -> Self {
        let seed = config.seed();
        let transform = ContinentTransform::from_seed(
            // Dedicated stream so island knobs cannot move continents.
            crate::seed::mix64(seed.value() ^ u64::from(seed.stream("continent_layout"))),
            config.continent_drift_weight(),
            config.continent_variation_weight(),
        );
        Self {
            config,
            seed_warp: seed.stream("continent_warp"),
            seed_detail: seed.stream("continent_detail"),
            seed_macro: seed.stream("continent_macro"),
            seed_island: seed.stream("island"),
            seed_island_warp: seed.stream("island_warp"),
            seed_coast: seed.stream("coast"),
            transform,
            threshold: config.land_threshold(),
        }
    }

    pub fn config(&self) -> WorldConfig {
        self.config
    }

    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Raw world field at normalised coordinates, in `0.0..=1.0`.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let continent_var = self.config.continent_variation_weight();
        let roughness = self.config.roughness_weight();
        let islands = self.config.island_weight();
        let island_var = self.config.island_variation_weight();

        // --- Continents (own streams + transform) ---------------------------
        let (bu, bv) = self.transform.map(u, v);

        // Fine warp on the already-drifted layout so silhouettes are organic.
        let warp_amount = 0.015 + continent_var * 0.12;
        let wx = fbm_signed(self.seed_warp, bu * 2.1, bv * 2.1) * warp_amount;
        let wy = fbm_signed(self.seed_warp, bu * 2.1 + 5.2, bv * 2.1 - 3.1) * warp_amount;
        let su = (bu + wx).rem_euclid(1.0);
        let sv = (bv + wy).clamp(0.0, 1.0);

        let bias = continental_bias(su, sv) * latitude_falloff(v);

        // Low-frequency macro field — this is what makes seed changes relocate
        // major masses instead of only fraying coasts.
        let macro_field = fbm(self.seed_macro, su * 1.15 + 2.7, sv * 1.15 - 1.4);
        let mix = self.transform.macro_mix;
        let mut height = bias * (1.0 - mix) + macro_field * mix;

        // Mid-scale continent detail.
        let broad = fbm(self.seed_detail, su * 1.9, sv * 1.9);
        height = height * (1.0 - continent_var * 0.35) + broad * (continent_var * 0.35);

        // Coastline detail (shared look, but does not move continents).
        if roughness > 0.0 {
            let fine = value_noise(self.seed_coast, su * 26.0, sv * 26.0) - 0.5;
            let mid = fbm(self.seed_detail, su * 7.0 + 11.0, sv * 7.0) - 0.5;
            height += fine * roughness * 0.15 + mid * roughness * 0.06;
        }

        // --- Islands (independent streams + knobs) --------------------------
        // Sampled in original (u,v) with their own warp so island clusters do
        // not inherit continent drift.
        if islands > 0.0 {
            let i_warp = 0.01 + island_var * 0.08;
            let iwx = fbm_signed(self.seed_island_warp, u * 3.1, v * 3.1) * i_warp;
            let iwy = fbm_signed(self.seed_island_warp, u * 3.1 + 8.0, v * 3.1) * i_warp;
            let iu = (u + iwx).rem_euclid(1.0);
            let iv = (v + iwy).clamp(0.0, 1.0);

            // Prefer open ocean relative to the *continent* field so islands
            // appear offshore rather than as continent texture.
            let open = (1.0 - bias / 0.48).clamp(0.0, 1.0);
            if open > 0.0 {
                let freq = 36.0 + island_var * 28.0;
                let speck = value_noise(self.seed_island, iu * freq, iv * freq);
                // Higher island_variation lowers the ridge threshold → more
                // shape diversity; density scales amplitude/gating.
                let ridge_cut = 0.78 - island_var * 0.12;
                let ridge = (speck - ridge_cut).max(0.0) / (1.0 - ridge_cut).max(1e-3);
                let cluster = fbm(self.seed_island, iu * 4.5 + 3.0, iv * 4.5);
                let gate = open * (0.35 + cluster * 0.65);
                height += ridge * gate * islands * (0.28 + island_var * 0.22);
            }
        }

        height.clamp(0.0, 1.0)
    }

    pub fn is_land(&self, u: f32, v: f32) -> bool {
        self.sample(u, v) > self.threshold
    }

    #[inline]
    pub fn world_coord(&self, x: u32, y: u32, width: u32, height: u32) -> (f32, f32) {
        let (fit_w, fit_h, off_x, off_y) = self.fit(width, height);
        let u = (x as f32 + 0.5 - off_x) / fit_w;
        let v = (y as f32 + 0.5 - off_y) / fit_h;
        (u, v)
    }

    pub fn fit(&self, width: u32, height: u32) -> (f32, f32, f32, f32) {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let side = w.min(h);
        (side, side, (w - side) * 0.5, (h - side) * 0.5)
    }

    pub fn in_world(&self, x: u32, y: u32, width: u32, height: u32) -> bool {
        let (fit_w, _, off_x, off_y) = self.fit(width, height);
        let fx = x as f32 + 0.5 - off_x;
        let fy = y as f32 + 0.5 - off_y;
        fx >= 0.0 && fy >= 0.0 && fx < fit_w && fy < fit_w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_is_deterministic() {
        let field = Field::new(WorldConfig::default());
        for i in 0..500 {
            let u = (i as f32) * 0.0019;
            let v = (i as f32) * 0.0037;
            assert_eq!(field.sample(u, v), field.sample(u, v));
        }
    }

    #[test]
    fn seeds_produce_different_worlds() {
        let config = |seed: u64| WorldConfig {
            seed,
            ..WorldConfig::default()
        };
        let fa = Field::new(config(1));
        let fb = Field::new(config(2));

        let mut differing = 0;
        for i in 0..400 {
            let u = (i % 20) as f32 / 20.0;
            let v = (i / 20) as f32 / 20.0;
            if (fa.sample(u, v) - fb.sample(u, v)).abs() > 1e-4 {
                differing += 1;
            }
        }
        assert!(differing > 300, "only {differing}/400 samples differed");
    }

    #[test]
    fn seeds_move_macro_land_masses() {
        // Coarse 8x8 land masks for two seeds should disagree in several cells.
        // This is the user-facing bug: continents stayed glued to the same spots.
        let mask = |seed: u64| -> [bool; 64] {
            let field = Field::new(WorldConfig {
                seed,
                ..WorldConfig::default()
            });
            let mut out = [false; 64];
            for y in 0..8 {
                for x in 0..8 {
                    let u = (x as f32 + 0.5) / 8.0;
                    let v = (y as f32 + 0.5) / 8.0;
                    out[y * 8 + x] = field.is_land(u, v);
                }
            }
            out
        };

        let a = mask(11);
        let b = mask(99);
        let flips = a.iter().zip(b.iter()).filter(|(x, y)| x != y).count();
        assert!(
            flips >= 8,
            "expected macro land mask to move across seeds, flips={flips}"
        );
    }

    #[test]
    fn continent_controls_change_silhouette_more_than_island_controls() {
        let base = WorldConfig {
            seed: 42,
            island_density: 0,
            ..WorldConfig::default()
        };
        let continent = WorldConfig {
            continent_variation: 100,
            continent_drift: 100,
            island_density: 0,
            ..base
        };
        let islands_only = WorldConfig {
            continent_variation: base.continent_variation,
            continent_drift: base.continent_drift,
            island_density: 100,
            island_variation: 100,
            ..base
        };

        let diff = |a: WorldConfig, b: WorldConfig| -> f32 {
            let fa = Field::new(a);
            let fb = Field::new(b);
            let mut sum = 0.0f32;
            for y in 0..32 {
                for x in 0..32 {
                    let u = (x as f32 + 0.5) / 32.0;
                    let v = (y as f32 + 0.5) / 32.0;
                    sum += (fa.sample(u, v) - fb.sample(u, v)).abs();
                }
            }
            sum
        };

        let continent_delta = diff(base, continent);
        let island_delta = diff(base, islands_only);
        assert!(
            continent_delta > island_delta * 1.15,
            "continent knobs should reshape more than island knobs: {continent_delta} vs {island_delta}"
        );
    }

    #[test]
    fn island_density_adds_offshore_land_without_identical_continents() {
        let none = WorldConfig {
            seed: 7,
            island_density: 0,
            island_variation: 0,
            ..WorldConfig::default()
        };
        let many = WorldConfig {
            island_density: 100,
            island_variation: 80,
            ..none
        };
        let fa = Field::new(none);
        let fb = Field::new(many);

        let mut gained = 0;
        for y in 0..48 {
            for x in 0..48 {
                let u = (x as f32 + 0.5) / 48.0;
                let v = (y as f32 + 0.5) / 48.0;
                if !fa.is_land(u, v) && fb.is_land(u, v) {
                    gained += 1;
                }
            }
        }
        assert!(
            gained > 5,
            "expected new island land cells, gained={gained}"
        );
    }

    #[test]
    fn samples_stay_in_unit_range() {
        let field = Field::new(WorldConfig {
            continent_variation: 100,
            continent_drift: 100,
            island_variation: 100,
            island_density: 100,
            coast_roughness: 100,
            ..WorldConfig::default()
        });
        for y in 0..64 {
            for x in 0..64 {
                let u = x as f32 / 64.0;
                let v = y as f32 / 64.0;
                let s = field.sample(u, v);
                assert!((0.0..=1.0).contains(&s), "{s} escaped 0..1 at {u},{v}");
            }
        }
    }

    #[test]
    fn coordinate_is_a_pure_function_of_global_position() {
        let field = Field::new(WorldConfig::default());
        let (w, h) = (512u32, 512u32);
        for (x, y) in [(0, 0), (17, 33), (255, 256), (511, 511)] {
            let direct = field.world_coord(x, y, w, h);
            let again = field.world_coord(x, y, w, h);
            assert_eq!(direct, again);
            assert_eq!(
                field.is_land(direct.0, direct.1),
                field.is_land(again.0, again.1)
            );
        }
    }

    #[test]
    fn aspect_ratio_is_preserved_and_centred() {
        let field = Field::new(WorldConfig::default());
        let (fit_w, fit_h, off_x, off_y) = field.fit(1000, 400);
        assert_eq!(fit_w, 400.0);
        assert_eq!(fit_h, 400.0);
        assert_eq!(off_x, 300.0);
        assert_eq!(off_y, 0.0);
        assert!(!field.in_world(0, 200, 1000, 400));
        assert!(field.in_world(500, 200, 1000, 400));
    }

    #[test]
    fn continental_bias_has_both_land_and_ocean() {
        let mut land = 0;
        let mut ocean = 0;
        for y in 0..40 {
            for x in 0..40 {
                let b = continental_bias(x as f32 / 40.0, y as f32 / 40.0);
                if b > 0.6 {
                    land += 1;
                }
                if b < 0.2 {
                    ocean += 1;
                }
            }
        }
        assert!(land > 100, "control field has too little land bias");
        assert!(ocean > 50, "control field has too little ocean");
    }
}
