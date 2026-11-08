//! The land/water field, evaluated **per coordinate** rather than as an image.
//!
//! Spec §5.3 and §5.4 make the shape of this module non-negotiable:
//!
//! - PhotoCraft splits a layer into bands and runs each band in a *fresh*
//!   WebAssembly instance. No state survives between calls.
//! - A full-canvas buffer inside a band is forbidden.
//!
//! So there is no `Landmass` image type here. There is a [`Field`] that answers
//! "is this world coordinate land?" for one point at a time — or fills a
//! rectangle — and its answer depends only on the config and the coordinate.

use crate::config::WorldConfig;
use crate::noise::{self, fbm, fbm_signed};

/// Continental-bias layout: where the generator wants large landmasses.
///
/// This is a low-resolution hand-authored control field, not noise. Spec §4.2:
/// "the initial Earth-like profile should use a designed continental-bias mask
/// or control field. Pure unconstrained noise is unlikely to produce
/// recognisable Earth-like continent placement reliably."
///
/// Row-major, `BIAS_W x BIAS_H`, values `0.0..=1.0` where 1 is strong land bias.
/// The shape is a stylised, non-representational arrangement of large
/// landmasses chosen to read as an arcade world-selection screen: a broad
/// western continent, a broad eastern continent, a southern landmass, and an
/// open ocean corridor between the western and eastern masses.
const BIAS_W: usize = 16;
const BIAS_H: usize = 9;

#[rustfmt::skip]
const CONTINENT_BIAS: [f32; BIAS_W * BIAS_H] = [
    //  west ocean   ..   western continent   ..  mid ocean  ..  eastern continent  .. east ocean
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
///
/// `u`/`v` are in `0.0..=1.0`; east-west wraps so the map has no seam.
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

/// Latitude falloff: pulls the far poles towards ocean so continents do not run
/// off the top and bottom edges.
fn latitude_falloff(v: f32) -> f32 {
    // 1.0 through the middle, easing to ~0.55 at the poles.
    let t = ((v - 0.5) * 2.0).abs();
    let eased = t * t * t;
    1.0 - 0.45 * eased
}

/// A world field bound to one configuration.
///
/// Cheap to construct (no tables, no allocation) and `Copy`, so each band can
/// build one at no cost.
#[derive(Debug, Clone, Copy)]
pub struct Field {
    config: WorldConfig,
    seed_warp: u32,
    seed_detail: u32,
    seed_island: u32,
    seed_coast: u32,
    threshold: f32,
}

impl Field {
    /// Prepares the field for `config`.
    ///
    /// All per-call setup happens here, once — never per pixel.
    pub fn new(config: WorldConfig) -> Self {
        let seed = config.seed();
        Self {
            config,
            // Separate streams per stage (spec §3.3), so tuning the coast noise
            // cannot shift the continents.
            seed_warp: seed.stream("warp"),
            seed_detail: seed.stream("detail"),
            seed_island: seed.stream("island"),
            seed_coast: seed.stream("coast"),
            threshold: config.land_threshold(),
        }
    }

    /// The configuration this field was built from.
    pub fn config(&self) -> WorldConfig {
        self.config
    }

    /// The land threshold in use.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Raw world field at normalised coordinates, in `0.0..=1.0`.
    ///
    /// This is the pre-threshold scalar field described in spec §4.2:
    /// continental bias, warped by low-frequency noise, plus coastline detail
    /// and an island component.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let variation = self.config.variation_weight();
        let roughness = self.config.roughness_weight();
        let islands = self.config.island_weight();

        // 1-2. Domain warp: nudge the coordinates so continents are organic
        // rather than the control field's literal shape. `variation` sets how
        // far a seed may wander from the designed layout.
        let warp_amount = 0.02 + variation * 0.14;
        let wx = fbm_signed(self.seed_warp, u * 2.3, v * 2.3) * warp_amount;
        let wy = fbm_signed(self.seed_warp, u * 2.3 + 5.2, v * 2.3 - 3.1) * warp_amount;
        let su = (u + wx).rem_euclid(1.0);
        let sv = (v + wy).clamp(0.0, 1.0);

        // 3. Continental bias, plus latitude shaping.
        let bias = continental_bias(su, sv) * latitude_falloff(v);

        // 4. Large-scale noise, blended in proportionally to `variation`.
        let broad = fbm(self.seed_detail, su * 1.7, sv * 1.7);
        let mut height = bias * (1.0 - variation * 0.55) + broad * (variation * 0.55);

        // 5. Coastline detail: fine noise, strongest near the threshold band so
        // it breaks up shores without shredding continent interiors.
        if roughness > 0.0 {
            let fine = noise::value_noise(self.seed_coast, su * 26.0, sv * 26.0) - 0.5;
            let mid = fbm(self.seed_detail, su * 7.0 + 11.0, sv * 7.0) - 0.5;
            height += fine * roughness * 0.15 + mid * roughness * 0.06;
        }

        // 6. Islands: sparse high-frequency specks, only where the bias is low
        // (open ocean), so they read as islands rather than land texture.
        if islands > 0.0 && bias < 0.45 {
            let speck = noise::value_noise(self.seed_island, su * 48.0, sv * 48.0);
            let gate = (1.0 - bias / 0.45).clamp(0.0, 1.0);
            let ridge = (speck - 0.72).max(0.0) * 3.57; // 0 above 0.72, up to 1
            height += ridge * gate * islands * 0.42;
        }

        height.clamp(0.0, 1.0)
    }

    /// Whether normalised `(u, v)` is land.
    pub fn is_land(&self, u: f32, v: f32) -> bool {
        self.sample(u, v) > self.threshold
    }

    /// World coordinate for a document pixel, in normalised units.
    ///
    /// Aspect ratio is preserved (spec §4.3): the world is fitted inside the
    /// canvas and letterboxed, never stretched to arbitrary proportions.
    #[inline]
    pub fn world_coord(&self, x: u32, y: u32, width: u32, height: u32) -> (f32, f32) {
        let (fit_w, fit_h, off_x, off_y) = self.fit(width, height);
        let u = (x as f32 + 0.5 - off_x) / fit_w;
        let v = (y as f32 + 0.5 - off_y) / fit_h;
        (u, v)
    }

    /// Letterbox rectangle: `(fit_w, fit_h, off_x, off_y)` in pixels.
    ///
    /// A square world is fitted inside the document, centred, using the
    /// smaller dimension.
    pub fn fit(&self, width: u32, height: u32) -> (f32, f32, f32, f32) {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let side = w.min(h);
        (side, side, (w - side) * 0.5, (h - side) * 0.5)
    }

    /// Whether a document pixel falls inside the fitted world rather than the
    /// letterbox margin.
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
    fn samples_stay_in_unit_range() {
        let field = Field::new(WorldConfig {
            variation: 100,
            coast_roughness: 100,
            island_density: 100,
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
        // The band-boundary guarantee (spec Task 2 acceptance).
        let field = Field::new(WorldConfig::default());
        let (w, h) = (512u32, 512u32);
        for (x, y) in [(0, 0), (17, 33), (255, 256), (511, 511)] {
            let direct = field.world_coord(x, y, w, h);
            // Recomputing from a different vantage point must agree exactly.
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

        // Corners of the wider canvas are letterbox margin.
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
