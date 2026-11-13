//! The plug-in manifest, as JSON the host parses.
//!
//! Spec §5.2: reverse-DNS id, `kind: "filter"`, `area: "canvas"`, and parameter
//! definitions matching ABI v1. `generator_type` and `palette` are independent
//! choices: palette is color style only and applies to every generator.

use worldforge_core::config::{
    COAST_ROUGHNESS, CONTINENT_DRIFT, CONTINENT_VARIATION, ISLAND_DENSITY, ISLAND_VARIATION,
    LAND_COVERAGE, MOUNTAIN_DENSITY, PIXEL_SCALE, SEED,
};

/// Reverse-DNS plug-in id.
pub const PLUGIN_ID: &str = "org.worldforge.worldforge";

/// Menu label.
pub const PLUGIN_NAME: &str = "WorldForge";

/// Version reported in the manifest.
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The manifest JSON, built at compile time.
pub fn manifest_json() -> &'static str {
    MANIFEST
}

/// A single parameter definition, for tests and docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    pub key: &'static str,
    pub kind: &'static str,
    pub min: i32,
    pub max: i32,
    pub default: i32,
    pub unit_px: bool,
}

/// The numeric parameters, in dialog order.
pub const PARAMS: [Param; 9] = [
    Param {
        key: "seed",
        kind: "int",
        min: SEED.min,
        max: SEED.max,
        default: SEED.default,
        unit_px: false,
    },
    Param {
        key: "continent_variation",
        kind: "int",
        min: CONTINENT_VARIATION.min,
        max: CONTINENT_VARIATION.max,
        default: CONTINENT_VARIATION.default,
        unit_px: false,
    },
    Param {
        key: "continent_drift",
        kind: "int",
        min: CONTINENT_DRIFT.min,
        max: CONTINENT_DRIFT.max,
        default: CONTINENT_DRIFT.default,
        unit_px: false,
    },
    Param {
        key: "island_variation",
        kind: "int",
        min: ISLAND_VARIATION.min,
        max: ISLAND_VARIATION.max,
        default: ISLAND_VARIATION.default,
        unit_px: false,
    },
    Param {
        key: "island_density",
        kind: "int",
        min: ISLAND_DENSITY.min,
        max: ISLAND_DENSITY.max,
        default: ISLAND_DENSITY.default,
        unit_px: false,
    },
    Param {
        key: "coast_roughness",
        kind: "int",
        min: COAST_ROUGHNESS.min,
        max: COAST_ROUGHNESS.max,
        default: COAST_ROUGHNESS.default,
        unit_px: false,
    },
    Param {
        key: "mountain_density",
        kind: "int",
        min: MOUNTAIN_DENSITY.min,
        max: MOUNTAIN_DENSITY.max,
        default: MOUNTAIN_DENSITY.default,
        unit_px: false,
    },
    Param {
        key: "land_coverage",
        kind: "int",
        min: LAND_COVERAGE.min,
        max: LAND_COVERAGE.max,
        default: LAND_COVERAGE.default,
        unit_px: false,
    },
    Param {
        key: "pixel_scale",
        kind: "int",
        min: PIXEL_SCALE.min,
        max: PIXEL_SCALE.max,
        default: PIXEL_SCALE.default,
        unit_px: true,
    },
];

/// The manifest. See the module docs for the shape.
pub const MANIFEST: &str = r#"{
  "id": "org.worldforge.worldforge",
  "name": "WorldForge",
  "version": "0.1.0",
  "kind": "filter",
  "author": "WorldForge",
  "description": "Deterministic world generators with independent color palettes.",
  "params": {
    "generator_type":      {"type": "choice", "options": ["random_world", "procedural_world"], "default": "random_world"},
    "palette":             {"type": "choice", "options": ["arcade_blue_gold"], "default": "arcade_blue_gold"},
    "seed":                {"type": "int", "min": 0,  "max": 2147483647, "default": 123456},
    "continent_variation": {"type": "int", "min": 0,  "max": 100,        "default": 55},
    "continent_drift":     {"type": "int", "min": 0,  "max": 100,        "default": 65},
    "island_variation":    {"type": "int", "min": 0,  "max": 100,        "default": 40},
    "island_density":      {"type": "int", "min": 0,  "max": 100,        "default": 30},
    "coast_roughness":     {"type": "int", "min": 0,  "max": 100,        "default": 45},
    "mountain_density":    {"type": "int", "min": 0,  "max": 100,        "default": 55},
    "land_coverage":       {"type": "int", "min": 5,  "max": 90,         "default": 38},
    "pixel_scale":         {"type": "int", "min": 1,  "max": 8,          "default": 2, "unit": "px"},
    "generate_high_res":   {"type": "bool", "default": false},
    "coast_glow":          {"type": "bool", "default": true}
  },
  "overlap": 0,
  "area": "canvas"
}"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn json() -> serde_json::Value {
        serde_json::from_str(MANIFEST).expect("manifest must be valid JSON")
    }

    #[test]
    fn manifest_is_valid_json() {
        let v = json();
        assert_eq!(v["id"], PLUGIN_ID);
        assert_eq!(v["name"], PLUGIN_NAME);
        assert_eq!(v["kind"], "filter");
        assert_eq!(v["area"], "canvas");
    }

    #[test]
    fn id_is_reverse_dns_and_legal() {
        assert!(PLUGIN_ID.len() <= 64);
        assert!(PLUGIN_ID.contains('.'));
        assert!(
            PLUGIN_ID
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        );
    }

    #[test]
    fn every_param_matches_the_core_bounds() {
        let v = json();
        let params = v["params"].as_object().expect("params object");

        for p in PARAMS {
            let entry = params
                .get(p.key)
                .unwrap_or_else(|| panic!("missing {}", p.key));
            assert_eq!(entry["type"], p.kind, "{} type", p.key);
            assert_eq!(
                entry["min"].as_i64().unwrap(),
                i64::from(p.min),
                "{} min",
                p.key
            );
            assert_eq!(
                entry["max"].as_i64().unwrap(),
                i64::from(p.max),
                "{} max",
                p.key
            );
            assert_eq!(
                entry["default"].as_i64().unwrap(),
                i64::from(p.default),
                "{} default",
                p.key
            );
        }
    }

    #[test]
    fn continent_and_island_knobs_are_both_exposed() {
        let v = json();
        assert!(v["params"].get("continent_variation").is_some());
        assert!(v["params"].get("continent_drift").is_some());
        assert!(v["params"].get("island_variation").is_some());
        assert!(v["params"].get("island_density").is_some());
        assert!(v["params"].get("variation").is_none());
    }

    #[test]
    fn generator_type_and_palette_are_independent_choices() {
        let v = json();
        assert_eq!(v["params"]["generator_type"]["type"], "choice");
        assert_eq!(v["params"]["generator_type"]["default"], "random_world");
        let gens = v["params"]["generator_type"]["options"]
            .as_array()
            .expect("generator options");
        assert!(gens.iter().any(|g| g == "random_world"));
        assert!(gens.iter().any(|g| g == "procedural_world"));

        assert_eq!(v["params"]["palette"]["type"], "choice");
        assert_eq!(v["params"]["palette"]["default"], "arcade_blue_gold");
        assert!(v["params"].get("mountain_density").is_some());
    }

    #[test]
    fn pixel_scale_is_declared_in_px() {
        let v = json();
        assert_eq!(v["params"]["pixel_scale"]["unit"], "px");
        for p in PARAMS.iter().filter(|p| !p.unit_px) {
            assert!(
                v["params"][p.key].get("unit").is_none(),
                "{} must not declare a unit",
                p.key
            );
        }
    }

    #[test]
    fn bool_and_choice_params_match_spec_defaults() {
        let v = json();
        assert_eq!(v["params"]["generate_high_res"]["type"], "bool");
        assert_eq!(v["params"]["generate_high_res"]["default"], false);
        assert!(
            v["params"].get("randomize_settings").is_none(),
            "randomize must not be a dialog checkbox; PhotoCraft ABI has no action button"
        );
        assert_eq!(v["params"]["coast_glow"]["type"], "bool");
        assert_eq!(v["params"]["coast_glow"]["default"], true);
        assert_eq!(v["params"]["palette"]["type"], "choice");
        assert_eq!(v["params"]["palette"]["default"], "arcade_blue_gold");
    }

    #[test]
    fn every_param_range_is_representable_in_the_abi() {
        for p in PARAMS {
            assert!(
                p.min <= p.max,
                "{} has min {} > max {}",
                p.key,
                p.min,
                p.max
            );
            assert!(
                (p.min..=p.max).contains(&p.default),
                "{} default {} is outside {}..={}",
                p.key,
                p.default,
                p.min,
                p.max
            );
        }
    }
}
