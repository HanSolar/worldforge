//! The plug-in manifest, as JSON the host parses.
//!
//! Spec §5.2: reverse-DNS id, `kind: "filter"`, `area: "canvas"`, and parameter
//! definitions matching ABI v1. The control set is exactly spec §4.4 — the
//! dialog is deliberately small.

use worldforge_core::config::{
    COAST_ROUGHNESS, ISLAND_DENSITY, LAND_COVERAGE, PIXEL_SCALE, SEED, VARIATION,
};

/// Reverse-DNS plug-in id.
pub const PLUGIN_ID: &str = "org.worldforge.earth-arcade";

/// Menu label.
pub const PLUGIN_NAME: &str = "WorldForge: Earth Arcade";

/// Version reported in the manifest.
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The manifest JSON, built at compile time.
///
/// Handwritten as a string rather than serialised at runtime: the host reads it
/// before anything else runs, it must be byte-stable, and it saves pulling a
/// serializer into the wasm module.
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
///
/// Kept as data so tests can assert the JSON and the core agree without parsing
/// the string twice.
pub const PARAMS: [Param; 6] = [
    Param {
        key: "seed",
        kind: "int",
        min: SEED.min,
        max: SEED.max,
        default: SEED.default,
        unit_px: false,
    },
    Param {
        key: "variation",
        kind: "int",
        min: VARIATION.min,
        max: VARIATION.max,
        default: VARIATION.default,
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
        key: "island_density",
        kind: "int",
        min: ISLAND_DENSITY.min,
        max: ISLAND_DENSITY.max,
        default: ISLAND_DENSITY.default,
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
        // A block size in document pixels, so the host halves it for
        // low-resolution previews and the preview matches the applied result.
        unit_px: true,
    },
];

/// The manifest. See the module docs for the shape.
pub const MANIFEST: &str = r#"{
  "id": "org.worldforge.earth-arcade",
  "name": "WorldForge: Earth Arcade",
  "version": "0.1.0",
  "kind": "filter",
  "author": "WorldForge",
  "description": "Generates a deterministic, Earth-like arcade world map on the canvas.",
  "params": {
    "seed":           {"type": "int", "min": 0,  "max": 2147483647, "default": 123456},
    "variation":      {"type": "int", "min": 0,  "max": 100,        "default": 35},
    "coast_roughness":{"type": "int", "min": 0,  "max": 100,        "default": 45},
    "island_density": {"type": "int", "min": 0,  "max": 100,        "default": 30},
    "land_coverage":  {"type": "int", "min": 5,  "max": 90,         "default": 38},
    "pixel_scale":    {"type": "int", "min": 1,  "max": 8,          "default": 2, "unit": "px"},
    "coast_glow":     {"type": "bool", "default": true},
    "palette":        {"type": "choice", "options": ["arcade_blue_gold"], "default": "arcade_blue_gold"}
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
        // Host rules: 1-64 chars from A-Za-z0-9.-_
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
    fn pixel_scale_is_declared_in_px() {
        let v = json();
        assert_eq!(v["params"]["pixel_scale"]["unit"], "px");
        // Every other numeric param is unitless.
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
        assert_eq!(v["params"]["coast_glow"]["type"], "bool");
        assert_eq!(v["params"]["coast_glow"]["default"], true);
        assert_eq!(v["params"]["palette"]["type"], "choice");
        assert_eq!(v["params"]["palette"]["default"], "arcade_blue_gold");
    }

    #[test]
    fn every_param_range_is_representable_in_the_abi() {
        // The host validates `int` params as i32, so no declared bound may sit
        // outside i32. A vacuous `SEED.max <= i32::MAX` check would compile away;
        // assert the real invariant instead.
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
