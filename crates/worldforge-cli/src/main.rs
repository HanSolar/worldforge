//! `worldforge` — headless map generation.
//!
//! Spec §3.1: "The CLI is not required to ship Milestone 1." It exists here as
//! the fastest way to check a seed without PhotoCraft in the loop.
//!
//! ```text
//! worldforge --seed 123456 --size 512
//! worldforge --manifest
//! worldforge --seed 42 --continent-drift 90 --png out/world.png
//! ```

use std::{fs, process::ExitCode};

use worldforge_core::{
    GeneratorType, WorldConfig, config_from_params, land_fraction, render_image,
};

mod png_writer;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("worldforge: {message}");
            eprintln!("try `worldforge --help`");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let options = Options::parse(&args)?;

    if options.help {
        println!("{USAGE}");
        return Ok(());
    }

    if options.manifest {
        println!("{}", worldforge_photocraft::manifest_json());
        return Ok(());
    }

    let config = options.to_config()?;
    let size = options.size.unwrap_or(512);

    if let Some(path) = &options.save_settings {
        fs::write(path, config_json(config))
            .map_err(|e| format!("failed to write settings `{path}`: {e}"))?;
        println!("saved settings to {path}");
    }

    let land = land_fraction(config, 128);
    println!(
        "seed {} → {}x{} canvas, land {:.1}%, palette {}",
        config.seed,
        size,
        size,
        land * 100.0,
        config.palette.as_str()
    );

    if let Some(path) = &options.png {
        let samples = render_image(config, size, size, 3);
        png_writer::write_rgb8(path, &samples, size, size)?;
        println!("wrote {path}");
    }

    Ok(())
}

const USAGE: &str = "\
worldforge — deterministic world map generators

USAGE:
    worldforge [OPTIONS]

OPTIONS:
        --seed <N>                 Reproduce a map (default 123456)
        --generator <NAME>         random_world | procedural_world
        --palette <NAME>           Color style (default arcade_blue_gold)
        --continent-variation <N>  0..100, continent silhouette reshape
        --continent-drift <N>      0..100, how far continents move from anchors
        --island-variation <N>     0..100, island shape/placement randomness
        --islands <N>              0..100, small-island frequency
        --variation <N>            Legacy: maps into continent + island knobs
        --roughness <N>            0..100, coastline irregularity
        --mountains <N>            0..100, procedural_world mountain density
        --coverage <N>             5..90, approximate land share in percent
        --scale <N>                1..8, pixel-art block size
        --full-res                 Generate full-resolution output (preview-256 is default)
        --randomize                Action: randomize seed + all numeric/bool map fields
        --load-settings <PATH>     Load map settings JSON
        --save-settings <PATH>     Save final map settings JSON
        --no-glow                  Disable the bright coastline treatment
        --size <N>                 Preview size in pixels (default 512)
        --png <PATH>               Write a PNG preview
        --manifest                 Print the PhotoCraft plug-in manifest
    -h, --help                     Show this help";

#[derive(Debug, Default)]
struct Options {
    seed: Option<u64>,
    generator: Option<String>,
    palette: Option<String>,
    continent_variation: Option<i32>,
    continent_drift: Option<i32>,
    island_variation: Option<i32>,
    variation: Option<i32>,
    roughness: Option<i32>,
    mountains: Option<i32>,
    islands: Option<i32>,
    coverage: Option<i32>,
    scale: Option<i32>,
    full_res: bool,
    randomize: bool,
    load_settings: Option<String>,
    save_settings: Option<String>,
    no_glow: bool,
    size: Option<u32>,
    png: Option<String>,
    manifest: bool,
    help: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => options.help = true,
                "--manifest" => options.manifest = true,
                "--full-res" => options.full_res = true,
                "--randomize" => options.randomize = true,
                "--load-settings" => options.load_settings = Some(value(&mut args, arg)?),
                "--save-settings" => options.save_settings = Some(value(&mut args, arg)?),
                "--no-glow" => options.no_glow = true,
                "--seed" => options.seed = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?),
                "--generator" => options.generator = Some(value(&mut args, arg)?),
                "--palette" => options.palette = Some(value(&mut args, arg)?),
                "--continent-variation" => {
                    options.continent_variation =
                        Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--continent-drift" => {
                    options.continent_drift =
                        Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--island-variation" => {
                    options.island_variation =
                        Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--variation" => {
                    options.variation = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--roughness" => {
                    options.roughness = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--mountains" => {
                    options.mountains = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--islands" => {
                    options.islands = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--coverage" => {
                    options.coverage = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--scale" => {
                    options.scale = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?)
                }
                "--size" => options.size = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?),
                "--png" => options.png = Some(value(&mut args, arg)?),
                other => return Err(format!("unknown argument `{other}`")),
            }
        }

        Ok(options)
    }

    fn to_config(&self) -> Result<WorldConfig, String> {
        let mut config = if let Some(path) = &self.load_settings {
            let text = fs::read_to_string(path)
                .map_err(|e| format!("failed to read settings `{path}`: {e}"))?;
            config_from_params(&text).map_err(|e| format!("invalid settings `{path}`: {e}"))?
        } else {
            WorldConfig::default()
        };

        if let Some(seed) = self.seed {
            config.seed = seed;
        }
        if let Some(kind) = &self.generator {
            config.generator_type =
                GeneratorType::parse(kind).ok_or_else(|| format!("unknown generator `{kind}`"))?;
        }
        if let Some(palette) = &self.palette {
            config.palette = worldforge_core::PaletteId::parse(palette)
                .map_err(|e| format!("invalid palette: {e}"))?;
        }

        let has_split = self.continent_variation.is_some()
            || self.continent_drift.is_some()
            || self.island_variation.is_some();
        if !has_split {
            if let Some(v) = self.variation {
                config = config.apply_legacy_variation(v);
            }
        }

        if let Some(v) = self.continent_variation {
            config.continent_variation = v;
        }
        if let Some(v) = self.continent_drift {
            config.continent_drift = v;
        }
        if let Some(v) = self.island_variation {
            config.island_variation = v;
        }
        if let Some(v) = self.roughness {
            config.coast_roughness = v;
        }
        if let Some(v) = self.mountains {
            config.mountain_density = v;
        }
        if let Some(v) = self.islands {
            config.island_density = v;
        }
        if let Some(v) = self.coverage {
            config.land_coverage = v;
        }
        if let Some(v) = self.scale {
            config.pixel_scale = v;
        }
        config.generate_high_res = self.full_res;
        config.randomize_settings = self.randomize;
        if self.randomize {
            config = config.randomized();
            config.randomize_settings = true;
        }
        if self.no_glow {
            config.coast_glow = false;
        }

        Ok(config.clamped())
    }
}

fn config_json(config: WorldConfig) -> String {
    serde_json::json!({
        "generator_type": config.generator_type.as_str(),
        "palette": config.palette.as_str(),
        "seed": config.seed,
        "continent_variation": config.continent_variation,
        "continent_drift": config.continent_drift,
        "island_variation": config.island_variation,
        "island_density": config.island_density,
        "coast_roughness": config.coast_roughness,
        "mountain_density": config.mountain_density,
        "land_coverage": config.land_coverage,
        "pixel_scale": config.pixel_scale,
        "preview_thumbnail_256": config.preview_thumbnail_256,
        "generate_high_res": config.generate_high_res,
        "randomize_settings": config.randomize_settings,
        "coast_glow": config.coast_glow
    })
    .to_string()
}

fn value(args: &mut impl Iterator<Item = impl AsRef<str>>, flag: &str) -> Result<String, String> {
    args.next()
        .map(|v| v.as_ref().to_string())
        .ok_or_else(|| format!("`{flag}` needs a value"))
}

fn bad(flag: &str) -> impl Fn(std::num::ParseIntError) -> String + '_ {
    move |_| format!("`{flag}` got a value that is not a number")
}
