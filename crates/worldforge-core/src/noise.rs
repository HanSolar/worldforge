//! Seeded value noise and fBm.
//!
//! Value noise (rather than gradient noise) is deliberate: the plugin runs in a
//! WebAssembly *interpreter* under an instruction budget (spec §5.4), so each
//! octave is kept to a handful of hashes and three lerps.
//!
//! Everything here is a pure function of `(seed, x, y)`. No lattice tables are
//! built, so there is no per-band setup and no state to lose between bands.

/// Integer hash (the `xxHash` avalanche). One hash per lattice corner.
#[inline]
fn avalanche(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    x
}

#[inline]
fn corner(seed: u32, x: i32, y: i32) -> u32 {
    let a = (x as u32).wrapping_mul(0x9E37_79B1) ^ seed.rotate_left(13);
    let b = (y as u32).wrapping_mul(0x85EB_CA77);
    avalanche(a ^ b)
}

/// Lattice value in `0.0..1.0`.
#[inline]
fn lattice(seed: u32, x: i32, y: i32) -> f32 {
    // 24 bits is exact in f32.
    ((corner(seed, x, y) >> 8) as f32) * (1.0 / 16_777_216.0)
}

/// Quintic fade, so first and second derivatives are continuous at the lattice.
#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Single-octave value noise in `0.0..1.0`.
#[inline]
pub fn value_noise(seed: u32, x: f32, y: f32) -> f32 {
    if !x.is_finite() || !y.is_finite() {
        return 0.5;
    }

    let xf = x.floor();
    let yf = y.floor();
    let ix = xf as i32;
    let iy = yf as i32;
    let fx = x - xf;
    let fy = y - yf;

    let u = fade(fx);
    let v = fade(fy);

    let n00 = lattice(seed, ix, iy);
    let n10 = lattice(seed, ix + 1, iy);
    let n01 = lattice(seed, ix, iy + 1);
    let n11 = lattice(seed, ix + 1, iy + 1);

    lerp(lerp(n00, n10, u), lerp(n01, n11, u), v)
}

/// Octaves used by the world field.
pub const OCTAVES: u32 = 4;
const LACUNARITY: f32 = 2.0;
const GAIN: f32 = 0.5;

/// Fractal Brownian motion, normalised to `0.0..1.0`.
///
/// Each octave is offset so they do not share a lattice origin.
#[inline]
pub fn fbm(seed: u32, x: f32, y: f32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut normaliser = 0.0;

    let mut octave = 0;
    while octave < OCTAVES {
        let offset = octave as f32 * 17.31;
        total += value_noise(seed, x * frequency + offset, y * frequency - offset) * amplitude;
        normaliser += amplitude;
        amplitude *= GAIN;
        frequency *= LACUNARITY;
        octave += 1;
    }

    // normaliser is a fixed 1.875 for these constants; guard anyway.
    if normaliser <= 0.0 {
        return 0.5;
    }
    (total / normaliser).clamp(0.0, 1.0)
}

/// Signed fBm centred on zero, in roughly `-0.5..0.5`. Used for domain warping.
#[inline]
pub fn fbm_signed(seed: u32, x: f32, y: f32) -> f32 {
    fbm(seed, x, y) - 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_deterministic() {
        for i in 0..64 {
            let x = i as f32 * 0.137;
            let y = i as f32 * 0.311;
            assert_eq!(value_noise(9, x, y), value_noise(9, x, y));
            assert_eq!(fbm(9, x, y), fbm(9, x, y));
        }
    }

    #[test]
    fn different_seeds_differ() {
        let a: f32 = (0..32).map(|i| fbm(1, i as f32 * 0.21, 0.5)).sum();
        let b: f32 = (0..32).map(|i| fbm(2, i as f32 * 0.21, 0.5)).sum();
        assert!((a - b).abs() > 1e-3);
    }

    #[test]
    fn output_stays_in_unit_range() {
        for seed in [0u32, 1, 12345, u32::MAX] {
            for i in 0..2000 {
                let x = (i as f32) * 0.0213 - 21.0;
                let y = (i as f32) * 0.0171 - 17.0;
                let n = fbm(seed, x, y);
                assert!((0.0..=1.0).contains(&n), "fbm {n} escaped 0..1");
            }
        }
    }

    #[test]
    fn lattice_points_interpolate_to_their_corner_values() {
        // At an integer coordinate the four fade weights collapse to one corner.
        let at = value_noise(5, 3.0, 7.0);
        assert!((at - lattice(5, 3, 7)).abs() < 1e-6);
    }

    #[test]
    fn non_finite_input_is_handled() {
        assert_eq!(value_noise(1, f32::NAN, 0.0), 0.5);
        assert_eq!(value_noise(1, 0.0, f32::INFINITY), 0.5);
    }
}
