//! The generator's contract, exercised from outside the crate.
//!
//! These complement the unit tests: they treat `worldforge-core` as a consumer
//! would — a host that knows only about configs, bands and coordinates.

use worldforge_core::{
    Band, Field, RenderOptions, WorldConfig, config_from_params, land_fraction, render_band,
    render_image,
};

fn options(canvas: u32) -> RenderOptions {
    RenderOptions {
        canvas_width: canvas,
        canvas_height: canvas,
        coast_glow: true,
        pixel_scale: 2,
        preview_thumbnail_256: false,
    }
}

#[test]
fn same_config_produces_identical_pixels() {
    let config = WorldConfig::default();
    let a = render_image(config, 64, 64, 3);
    let b = render_image(config, 64, 64, 3);
    assert_eq!(a, b);
}

#[test]
fn different_seeds_produce_different_pixels() {
    let a = render_image(
        WorldConfig {
            seed: 1,
            ..WorldConfig::default()
        },
        64,
        64,
        3,
    );
    let b = render_image(
        WorldConfig {
            seed: 2,
            ..WorldConfig::default()
        },
        64,
        64,
        3,
    );
    assert_ne!(a, b);
}

#[test]
fn different_seeds_change_coarse_land_masks() {
    // User-facing requirement: continents should not stay glued to the same
    // anchors when only the seed changes.
    let mask = |seed: u64| -> Vec<bool> {
        let field = Field::new(WorldConfig {
            seed,
            ..WorldConfig::default()
        });
        let mut cells = Vec::with_capacity(64);
        for y in 0..8 {
            for x in 0..8 {
                let u = (x as f32 + 0.5) / 8.0;
                let v = (y as f32 + 0.5) / 8.0;
                cells.push(field.is_land(u, v));
            }
        }
        cells
    };

    let flips = mask(11)
        .into_iter()
        .zip(mask(99))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        flips >= 8,
        "expected coarse land mask flips across seeds, got {flips}"
    );
}

#[test]
fn continent_knobs_reshape_more_than_island_knobs() {
    let base = WorldConfig {
        seed: 42,
        island_density: 0,
        ..WorldConfig::default()
    };
    let continent = WorldConfig {
        continent_variation: 100,
        continent_drift: 100,
        ..base
    };
    let islands = WorldConfig {
        island_density: 100,
        island_variation: 100,
        ..base
    };

    let score = |a: WorldConfig, b: WorldConfig| -> f32 {
        let fa = Field::new(a);
        let fb = Field::new(b);
        let mut sum = 0.0;
        for y in 0..32 {
            for x in 0..32 {
                let u = (x as f32 + 0.5) / 32.0;
                let v = (y as f32 + 0.5) / 32.0;
                sum += (fa.sample(u, v) - fb.sample(u, v)).abs();
            }
        }
        sum
    };

    let continent_delta = score(base, continent);
    let island_delta = score(base, islands);
    assert!(
        continent_delta > island_delta * 1.15,
        "continent controls should dominate island controls: {continent_delta} vs {island_delta}"
    );
}

#[test]
fn legacy_variation_params_still_parse() {
    let config = config_from_params(r#"{"seed":9,"variation":60}"#).expect("legacy params");
    assert_eq!(config.continent_variation, 60);
    assert_eq!(config.continent_drift, 60);
}

#[test]
fn splitting_a_render_into_bands_changes_nothing() {
    // The acceptance criterion for spec Task 2. Simulates the host's banding by
    // rendering a full image, then the same image as independent strips
    // rendered by fresh `Field`s (as a fresh wasm instance would).
    let config = WorldConfig::default();
    let (w, h, ch) = (96u32, 96u32, 4u32);

    let whole = render_image(config, w, h, ch);

    let mut stitched = vec![0.0f32; whole.len()];
    let strip_height = 24u32;
    for row in 0..(h / strip_height) {
        // A fresh field per band: no state carried over.
        let field = Field::new(config);
        let band = Band::new(0, row * strip_height, w, strip_height, ch);
        let mut buf = vec![0.0f32; band.sample_count()];
        render_band(&field, &band, &options(w), &mut buf);

        for y in 0..strip_height as usize {
            let src = y * w as usize * ch as usize;
            let dst = ((row as usize * strip_height as usize) + y) * w as usize * ch as usize;
            stitched[dst..dst + w as usize * ch as usize]
                .copy_from_slice(&buf[src..src + w as usize * ch as usize]);
        }
    }

    assert_eq!(whole, stitched, "banding altered the output");
}

#[test]
fn alpha_channel_is_never_written() {
    let config = WorldConfig::default();
    let (w, h, ch) = (32u32, 32u32, 4u32);

    let mut buf = vec![1.0f32; (w * h * ch) as usize];
    let field = Field::new(config);
    render_band(&field, &Band::new(0, 0, w, h, ch), &options(w), &mut buf);

    for i in (3..buf.len()).step_by(4) {
        assert_eq!(buf[i], 1.0, "alpha at sample {i} was modified");
    }
}

#[test]
fn land_coverage_parameter_moves_the_land_fraction() {
    let sparse = WorldConfig {
        land_coverage: 15,
        ..WorldConfig::default()
    };
    let dense = WorldConfig {
        land_coverage: 75,
        ..WorldConfig::default()
    };

    let sparse_land = land_fraction(sparse, 64);
    let dense_land = land_fraction(dense, 64);

    assert!(
        dense_land > sparse_land,
        "land_coverage 75 ({dense_land}) should exceed 15 ({sparse_land})"
    );
}

#[test]
fn default_config_lands_in_a_plausible_range() {
    // Not a visual judgement, just a guard against a regression that turns the
    // map into all-ocean or all-land.
    let fraction = land_fraction(WorldConfig::default(), 128);
    assert!(
        (0.15..0.65).contains(&fraction),
        "default land fraction {fraction} is implausible"
    );
}

#[test]
fn island_density_changes_the_map() {
    let none = WorldConfig {
        island_density: 0,
        ..WorldConfig::default()
    };
    let many = WorldConfig {
        island_density: 100,
        ..WorldConfig::default()
    };
    assert_ne!(render_image(none, 64, 64, 3), render_image(many, 64, 64, 3));
}

#[test]
fn coast_glow_toggle_changes_the_map() {
    let on = WorldConfig {
        coast_glow: true,
        ..WorldConfig::default()
    };
    let off = WorldConfig {
        coast_glow: false,
        ..WorldConfig::default()
    };
    assert_ne!(render_image(on, 64, 64, 3), render_image(off, 64, 64, 3));
}

#[test]
fn pixel_scale_produces_blocky_output() {
    // At scale 4, four adjacent pixels share a texel, so each 4x4 block should
    // be a single flat colour in the interior of the world.
    let config = WorldConfig {
        pixel_scale: 4,
        ..WorldConfig::default()
    };
    let size = 64u32;
    let field = Field::new(config);
    let opts = RenderOptions {
        canvas_width: size,
        canvas_height: size,
        coast_glow: false,
        pixel_scale: 4,
        preview_thumbnail_256: false,
    };

    let mut buf = vec![0.0f32; (size * size * 3) as usize];
    render_band(&field, &Band::new(0, 0, size, size, 3), &opts, &mut buf);

    let px = |x: u32, y: u32| -> [f32; 3] {
        let i = ((y * size + x) * 3) as usize;
        [buf[i], buf[i + 1], buf[i + 2]]
    };

    // Pick a block well inside the canvas.
    let (bx, by) = (20u32, 20u32);
    let expected = px(bx, by);
    for dy in 0..4 {
        for dx in 0..4 {
            assert_eq!(
                px(bx + dx, by + dy),
                expected,
                "pixel_scale block not flat at +({dx},{dy})"
            );
        }
    }
}

#[test]
fn params_json_is_parsed_and_clamped() {
    let json = r#"{"seed": 7, "land_coverage": 500, "pixel_scale": 0,
                   "_image": {"x": 0, "y": 0, "canvasWidth": 64, "canvasHeight": 64}}"#;
    let config = config_from_params(json).expect("params should parse");

    assert_eq!(config.seed, 7);
    assert_eq!(config.land_coverage, 90, "clamped to the max");
    assert_eq!(config.pixel_scale, 1, "clamped to the min");
}

#[test]
fn params_json_tolerates_the_host_image_block() {
    // `_image` and any future host keys must not break parsing.
    let json = r#"{"seed": 3, "_image": {"x": 0, "y": 0, "canvasWidth": 8, "canvasHeight": 8},
                   "some_future_key": [1, 2, 3]}"#;
    assert!(config_from_params(json).is_ok());
}

#[test]
fn malformed_params_are_an_error_not_a_panic() {
    assert!(config_from_params("{not json").is_err());
    assert!(config_from_params(r#"{"palette": "does_not_exist"}"#).is_err());
}

#[test]
fn non_square_canvas_is_letterboxed_not_stretched() {
    let config = WorldConfig::default();
    let field = Field::new(config);
    let opts = RenderOptions {
        canvas_width: 200,
        canvas_height: 100,
        coast_glow: true,
        pixel_scale: 1,
        preview_thumbnail_256: false,
    };

    let mut buf = vec![0.0f32; (200 * 100 * 3) as usize];
    render_band(&field, &Band::new(0, 0, 200, 100, 3), &opts, &mut buf);

    // The left and right edges must be margin (black), not map content.
    let px = |x: u32, y: u32| -> [f32; 3] {
        let i = ((y * 200 + x) * 3) as usize;
        [buf[i], buf[i + 1], buf[i + 2]]
    };

    assert_eq!(px(0, 50), [0.0, 0.0, 0.0], "left margin should be empty");
    assert_eq!(px(199, 50), [0.0, 0.0, 0.0], "right margin should be empty");

    // The centre must be actual map content.
    assert_ne!(px(100, 50), [0.0, 0.0, 0.0], "centre should hold the map");
}
