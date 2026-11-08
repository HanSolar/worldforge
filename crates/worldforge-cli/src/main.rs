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

use std::process::ExitCode;

use worldforge_core::{WorldConfig, land_fraction, render_image};

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
worldforge — deterministic Earth-like arcade map generator

USAGE:
    worldforge [OPTIONS]

OPTIONS:
        --seed <N>                 Reproduce a map (default 123456)
        --continent-variation <N>  0..100, continent silhouette reshape
        --continent-drift <N>      0..100, how far continents move from anchors
        --island-variation <N>     0..100, island shape/placement randomness
        --islands <N>              0..100, small-island frequency
        --variation <N>            Legacy: maps into continent + island knobs
        --roughness <N>            0..100, coastline irregularity
        --coverage <N>             5..90, approximate land share in percent
        --scale <N>                1..8, pixel-art block size
        --no-glow                  Disable the bright coastline treatment
        --size <N>                 Preview size in pixels (default 512)
        --png <PATH>               Write a PNG preview
        --manifest                 Print the PhotoCraft plug-in manifest
    -h, --help                     Show this help";

#[derive(Debug, Default)]
struct Options {
    seed: Option<u64>,
    continent_variation: Option<i32>,
    continent_drift: Option<i32>,
    island_variation: Option<i32>,
    variation: Option<i32>,
    roughness: Option<i32>,
    islands: Option<i32>,
    coverage: Option<i32>,
    scale: Option<i32>,
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
                "--no-glow" => options.no_glow = true,
                "--seed" => options.seed = Some(value(&mut args, arg)?.parse().map_err(bad(arg))?),
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
        let mut config = WorldConfig::default();

        if let Some(seed) = self.seed {
            config.seed = seed;
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
        if let Some(v) = self.islands {
            config.island_density = v;
        }
        if let Some(v) = self.coverage {
            config.land_coverage = v;
        }
        if let Some(v) = self.scale {
            config.pixel_scale = v;
        }
        if self.no_glow {
            config.coast_glow = false;
        }

        Ok(config.clamped())
    }
}

fn value(args: &mut impl Iterator<Item = impl AsRef<str>>, flag: &str) -> Result<String, String> {
    args.next()
        .map(|v| v.as_ref().to_string())
        .ok_or_else(|| format!("`{flag}` needs a value"))
}

fn bad(flag: &str) -> impl Fn(std::num::ParseIntError) -> String + '_ {
    move |_| format!("`{flag}` got a value that is not a number")
}
