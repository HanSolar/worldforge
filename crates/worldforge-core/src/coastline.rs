//! Land classification, speck cleanup, and coastline detection.
//!
//! Every function here is a pure function of the global world coordinate. The
//! neighbour lookups that coastline work needs are ordinary field samples at
//! offset coordinates, which is what makes band-split rendering seamless
//! (spec §5.3): a band never needs pixels outside itself, only coordinates.

use crate::landmass::Field;

/// A classified world texel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Ocean,
    Land,
}

/// How far from the threshold a sample must be before it skips the
/// neighbourhood majority vote.
///
/// Cleanup only runs on genuinely ambiguous samples, which keeps the cost near
/// zero on the unambiguous interior of a continent.
const CLEANUP_MARGIN: f32 = 0.02;

/// Classifies a world texel, removing isolated specks.
///
/// Spec §4.2 step 6 asks for cleanup passes against one-pixel noise. A literal
/// multi-pass buffer filter is impossible under banded execution (no state
/// survives, and a full-image buffer is forbidden), so this uses the
/// band-equivalent: a majority vote over the 3x3 neighbourhood, applied only
/// where the classification is marginal. The result is a pure function of the
/// coordinate, so every band reaches the same verdict.
pub fn classify(field: &Field, u: f32, v: f32) -> Surface {
    let height = field.sample(u, v);
    let threshold = field.threshold();

    if (height - threshold).abs() > CLEANUP_MARGIN {
        return if height > threshold {
            Surface::Land
        } else {
            Surface::Ocean
        };
    }

    // Marginal sample: vote with the neighbours. The step is one world texel.
    let step = texel_size();
    let mut land_votes = 0;
    let mut dy = -1;
    while dy <= 1 {
        let mut dx = -1;
        while dx <= 1 {
            let s = field.sample(u + dx as f32 * step, v + dy as f32 * step);
            if s > threshold {
                land_votes += 1;
            }
            dx += 1;
        }
        dy += 1;
    }

    // 5 of 9 keeps a lone speck from claiming a cell, and a lone hole from
    // breaking a coastline.
    if land_votes >= 5 {
        Surface::Land
    } else {
        Surface::Ocean
    }
}

/// One world texel in normalised units.
///
/// The world is sampled on a fixed virtual grid independent of document
/// resolution (spec §4.3), so output size changes detail density, not layout.
#[inline]
pub fn texel_size() -> f32 {
    1.0 / 512.0
}

/// Quantises a normalised coordinate to the world texel grid.
///
/// This is what makes `pixel_scale` work: several document pixels share one
/// world texel, producing blocky pixel-art edges.
#[inline]
pub fn snap(u: f32, v: f32) -> (f32, f32) {
    let step = texel_size();
    let su = ((u / step).floor() + 0.5) * step;
    let sv = ((v / step).floor() + 0.5) * step;
    (su, sv)
}

/// Whether this texel touches water, i.e. is on the coastline.
///
/// Coast detection is done on the *snapped* grid so the cyan edge follows the
/// pixel-art blocks instead of stair-stepping at document resolution.
pub fn is_coast(field: &Field, u: f32, v: f32) -> bool {
    if classify(field, u, v) != Surface::Land {
        return false;
    }

    let step = texel_size();
    // Four-neighbour test: cheap, and enough for a readable shore at pixel_scale
    // >= 2. At scale 1 it can drop single-texel spurs, which reads fine.
    let offsets = [(step, 0.0), (-step, 0.0), (0.0, step), (0.0, -step)];
    for (du, dv) in offsets {
        if classify(field, u + du, v + dv) == Surface::Ocean {
            return true;
        }
    }
    false
}

/// Distance in texels from a land texel to the nearest water, capped.
///
/// Used for the outer glow (spec §4.5). Returns `None` when not close enough to
/// shore to glow, which keeps the search cheap.
///
/// The returned distance never exceeds `max_texels`: ring `r` holds the samples
/// at Chebyshev distance `r`, and a square ring's corners are `r√2` away, so
/// corners past the cap are skipped rather than reported.
pub fn shore_distance(field: &Field, u: f32, v: f32, max_texels: i32) -> Option<f32> {
    let step = texel_size();
    let cap = max_texels.max(1);

    let mut r = 1;
    while r <= cap {
        // Sample the ring's perimeter at a stride that grows with r.
        let stride = (r / 2).max(1);
        let mut best: Option<f32> = None;

        let mut d = -r;
        while d <= r {
            let points = [(d, -r), (d, r), (-r, d), (r, d)];
            for (dx, dy) in points {
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist > cap as f32 {
                    continue;
                }
                if classify(field, u + dx as f32 * step, v + dy as f32 * step) == Surface::Ocean {
                    best = Some(best.map_or(dist, |b: f32| b.min(dist)));
                }
            }
            d += stride;
        }

        // The ring is the nearest shell, so the first hit is the answer.
        if let Some(dist) = best {
            return Some(dist);
        }
        r += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldConfig;

    fn field() -> Field {
        Field::new(WorldConfig::default())
    }

    #[test]
    fn classification_is_deterministic() {
        let f = field();
        for i in 0..200 {
            let u = i as f32 / 200.0;
            let v = (i * 7 % 200) as f32 / 200.0;
            assert_eq!(classify(&f, u, v), classify(&f, u, v));
        }
    }

    #[test]
    fn classification_is_band_independent() {
        // Same coordinate classified from two different "bands" must agree.
        let f = field();
        let coords: Vec<(f32, f32)> = (0..64)
            .map(|i| (i as f32 / 64.0, (i * 3 % 64) as f32 / 64.0))
            .collect();

        let band_a: Vec<Surface> = coords.iter().map(|(u, v)| classify(&f, *u, *v)).collect();
        let band_b: Vec<Surface> = coords
            .iter()
            .rev()
            .map(|(u, v)| classify(&f, *u, *v))
            .collect();
        let mut band_b_rev = band_b;
        band_b_rev.reverse();

        assert_eq!(band_a, band_b_rev);
    }

    #[test]
    fn snapping_quantises_to_the_texel_grid() {
        let step = texel_size();
        let (su, sv) = snap(0.5001, 0.7499);
        let q = |x: f32| (x / step).floor() + 0.5;
        assert!((su - q(0.5001) * step).abs() < 1e-6);
        assert!((sv - q(0.7499) * step).abs() < 1e-6);

        // Neighbouring points inside one texel snap to the same value.
        assert_eq!(snap(0.3, 0.3), snap(0.3 + step * 0.3, 0.3));
    }

    #[test]
    fn coast_only_reports_land() {
        let f = field();
        for i in 0..300 {
            let u = i as f32 / 300.0;
            let v = 0.5;
            if is_coast(&f, u, v) {
                assert_eq!(classify(&f, u, v), Surface::Land);
            }
        }
    }

    #[test]
    fn world_has_a_coastline() {
        let f = field();
        // A coastline is a one-texel-wide feature. On a 160x100 sample grid the
        // spacing is ~3.2 texels, so only a fraction of the perimeter can be
        // hit — a few dozen hits is the expected order of magnitude, not a bug.
        let samples = 16_000;
        let coastal = (0..samples)
            .filter(|i| {
                let u = (*i % 160) as f32 / 160.0;
                let v = (*i / 160) as f32 / 100.0;
                is_coast(&f, u, v)
            })
            .count();
        assert!(
            coastal > 40,
            "expected a visible coastline, found {coastal}"
        );
    }

    #[test]
    fn shore_distance_is_finite_and_bounded() {
        let f = field();
        for i in 0..200 {
            let u = i as f32 / 200.0;
            if let Some(d) = shore_distance(&f, u, 0.4, 8) {
                assert!((1.0..=8.0).contains(&d), "distance {d} out of bounds");
            }
        }
    }
}
