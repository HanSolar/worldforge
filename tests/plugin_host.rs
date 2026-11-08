//! Runs the built `.wasm` module the way PhotoCraft does.
//!
//! This is the closest thing to "the plug-in installs and runs" that can be
//! checked without the host application: the module is loaded into `wasmi` (the
//! same interpreter PhotoCraft uses), the manifest is read through the ABI, and a
//! band of pixels is filtered in place.
//!
//! It also verifies the property the banded execution model depends on: two
//! independent instances rendering adjacent bands must produce exactly what a
//! single instance rendering both produces.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use wasmi::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

fn ensure_wasm() -> PathBuf {
    static WASM: OnceLock<PathBuf> = OnceLock::new();
    WASM.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let path = root.join("target/wasm32-unknown-unknown/release/worldforge_photocraft.wasm");

        let status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "worldforge-photocraft",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
            ])
            .current_dir(&root)
            .status()
            .expect("spawn cargo build for wasm plugin");
        assert!(
            status.success(),
            "failed to build wasm artifact for host tests"
        );
        path
    })
    .clone()
}

fn wasm_bytes() -> Vec<u8> {
    let path = ensure_wasm();
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// A loaded plug-in instance, mirroring one band's fresh instance.
struct Plugin {
    store: Store<()>,
    instance: Instance,
}

impl Plugin {
    /// Instantiates the module. Fails if it has any import, exactly as the host
    /// would reject it.
    fn instantiate(bytes: &[u8]) -> Self {
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).expect("module must validate");

        let mut store = Store::new(&engine, ());
        let linker = Linker::<()>::new(&engine);

        // A linker with nothing registered can only succeed if the module
        // imports nothing.
        let instance = linker
            .instantiate(&mut store, &module)
            .expect("instantiation must not need imports")
            .start(&mut store)
            .expect("no start function should trap");

        Self { store, instance }
    }

    fn func(&mut self, name: &str) -> TypedFunc<(), i32> {
        self.instance
            .get_typed_func::<(), i32>(&self.store, name)
            .unwrap_or_else(|e| panic!("export `{name}`: {e}"))
    }

    fn memory(&mut self) -> Memory {
        self.instance
            .get_memory(&self.store, "memory")
            .expect("memory export")
    }

    /// Reads the manifest through `pc_manifest`.
    fn manifest(&mut self) -> String {
        let f = self
            .instance
            .get_typed_func::<(), i64>(&self.store, "pc_manifest")
            .expect("pc_manifest");
        let packed = f.call(&mut self.store, ()).expect("pc_manifest call");

        let len = (packed >> 32) as u32 as usize;
        let ptr = (packed & 0xFFFF_FFFF) as u32;

        let memory = self.memory();
        let mut buf = vec![0u8; len];
        memory
            .read(&self.store, ptr as usize, &mut buf)
            .expect("read manifest");
        String::from_utf8(buf).expect("manifest must be UTF-8")
    }

    /// Allocates `len` bytes inside the module.
    fn alloc(&mut self, len: i32) -> u32 {
        // A zero-byte request is legal from the host's side but the module
        // rejects it; round up so tests can still exercise zero-sized bands.
        let len = len.max(16);
        let f = self
            .instance
            .get_typed_func::<i32, i32>(&self.store, "pc_alloc")
            .expect("pc_alloc");
        let ptr = f.call(&mut self.store, len).expect("pc_alloc call");
        assert!(ptr != 0, "pc_alloc returned 0 for {len} bytes");
        ptr as u32
    }

    /// Runs `pc_filter` over a band, returning the resulting samples.
    fn run_band(&mut self, band: BandSpec, params: &str) -> Result<Vec<f32>, i32> {
        let pixel_bytes = (band.width * band.height * band.channels * 4) as i32;
        let params_bytes = params.as_bytes();

        let buf_ptr = self.alloc(pixel_bytes);
        let params_ptr = self.alloc(params_bytes.len() as i32);

        let memory = self.memory();
        (0..band.width * band.height)
            .flat_map(|_| (0..band.channels).map(|c| if c == 3 { 1.0f32 } else { 0.0f32 }))
            .enumerate()
            .try_for_each(|(i, v)| {
                memory.write(&mut self.store, buf_ptr as usize + i * 4, &v.to_le_bytes())
            })
            .expect("write pixels");
        memory
            .write(&mut self.store, params_ptr as usize, params_bytes)
            .expect("write params");

        // format = depth | alpha << 8 | mode << 16  (8-bit, alpha, RGB)
        let format = 8 | (1 << 8) | (3 << 16);

        let f = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32, i32, i32, i32, i32), i32>(
                &self.store,
                "pc_filter",
            )
            .expect("pc_filter");

        let code = f
            .call(
                &mut self.store,
                (
                    buf_ptr as i32,
                    pixel_bytes,
                    band.width as i32,
                    band.height as i32,
                    band.channels as i32,
                    format,
                    params_ptr as i32,
                    params_bytes.len() as i32,
                ),
            )
            .expect("pc_filter must not trap");

        if code != 0 {
            return Err(code);
        }

        let memory = self.memory();
        let mut samples = vec![0.0f32; (band.width * band.height * band.channels) as usize];
        for (i, slot) in samples.iter_mut().enumerate() {
            let mut raw = [0u8; 4];
            memory
                .read(&self.store, buf_ptr as usize + i * 4, &mut raw)
                .expect("read sample");
            *slot = f32::from_le_bytes(raw);
        }
        Ok(samples)
    }
}

#[derive(Clone, Copy)]
struct BandSpec {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    channels: u32,
}

impl BandSpec {
    fn params(self, canvas_w: u32, canvas_h: u32) -> String {
        format!(
            r#"{{"seed":123456,"continent_variation":55,"continent_drift":65,
                "island_variation":40,"island_density":30,"coast_roughness":45,
                "land_coverage":38,"pixel_scale":2,"coast_glow":true,"palette":"arcade_blue_gold",
                "_image":{{"x":{},"y":{},"width":{},"height":{},"overlap":0,
                          "canvasWidth":{canvas_w},"canvasHeight":{canvas_h},
                          "mode":"rgb","depth":8,"alpha":true,"channels":{}}}}}"#,
            self.x, self.y, self.width, self.height, self.channels
        )
    }
}

#[test]
fn plugin_instantiates_without_imports() {
    let bytes = wasm_bytes();
    let mut plugin = Plugin::instantiate(&bytes);

    let version = plugin
        .func("pc_abi_version")
        .call(&mut plugin.store, ())
        .unwrap();
    assert_eq!(version, 1, "pc_abi_version must report ABI v1");
}

#[test]
fn manifest_is_readable_through_the_abi() {
    let bytes = wasm_bytes();
    let mut plugin = Plugin::instantiate(&bytes);
    let manifest = plugin.manifest();

    let parsed: serde_json::Value = serde_json::from_str(&manifest).expect("manifest JSON");
    assert_eq!(parsed["id"], "org.worldforge.earth-arcade");
    assert_eq!(parsed["kind"], "filter");
    assert_eq!(parsed["area"], "canvas");
}

#[test]
fn filter_paints_a_band() {
    let bytes = wasm_bytes();
    let mut plugin = Plugin::instantiate(&bytes);

    let band = BandSpec {
        x: 0,
        y: 0,
        width: 32,
        height: 32,
        channels: 4,
    };
    let samples = plugin
        .run_band(band, &band.params(32, 32))
        .expect("filter ok");

    assert_eq!(samples.len(), 32 * 32 * 4);

    // Alpha was pre-set to 1.0 and must be untouched.
    for px in 0..32 * 32 {
        assert_eq!(samples[px * 4 + 3], 1.0, "alpha changed at pixel {px}");
    }

    // The map must not be uniform.
    let distinct: std::collections::BTreeSet<u32> =
        (0..32 * 32).map(|p| samples[p * 4].to_bits()).collect();
    assert!(
        distinct.len() > 2,
        "expected varied output, got {} values",
        distinct.len()
    );
}

#[test]
fn filter_rejects_bad_geometry() {
    let bytes = wasm_bytes();
    let mut plugin = Plugin::instantiate(&bytes);

    let band = BandSpec {
        x: 0,
        y: 0,
        width: 0,
        height: 32,
        channels: 4,
    };
    let err = plugin.run_band(band, &band.params(32, 32)).unwrap_err();
    assert_eq!(err, 3, "zero width should be a geometry error");
}

#[test]
fn filter_rejects_missing_geometry() {
    let bytes = wasm_bytes();
    let mut plugin = Plugin::instantiate(&bytes);

    // Params with no `_image` block: the plugin cannot know where it is.
    let band = BandSpec {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
        channels: 4,
    };
    let err = plugin
        .run_band(band, r#"{"seed":1,"pixel_scale":2}"#)
        .unwrap_err();
    assert_eq!(err, 3, "missing _image should be a geometry error");
}

#[test]
fn filter_rejects_malformed_params() {
    let bytes = wasm_bytes();
    let mut plugin = Plugin::instantiate(&bytes);

    let band = BandSpec {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
        channels: 4,
    };
    let err = plugin.run_band(band, "{not json").unwrap_err();
    assert_eq!(err, 2, "bad JSON should be a params error");
}

#[test]
fn filter_rejects_an_unsupported_colour_mode() {
    let bytes = wasm_bytes();
    let mut plugin = Plugin::instantiate(&bytes);

    let band = BandSpec {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
        channels: 4,
    };
    let params = band.params(8, 8);

    let pixel_bytes = 8 * 8 * 4 * 4;
    let buf_ptr = plugin.alloc(pixel_bytes);
    let params_ptr = plugin.alloc(params.len() as i32);
    let memory = plugin.memory();
    memory
        .write(&mut plugin.store, params_ptr as usize, params.as_bytes())
        .unwrap();

    // mode 2 = Indexed, which the host stores as RGB and this plug-in accepts;
    // modes 1/4/7/8/9 (Gray, CMYK, Multichannel, Duotone, Lab) do not share the
    // RGB channel layout, so they must be refused rather than filled with RGB.
    // 4 = CMYK here, which is genuinely unsupported.
    let format = 8 | (1 << 8) | (4 << 16);
    let f = plugin
        .instance
        .get_typed_func::<(i32, i32, i32, i32, i32, i32, i32, i32), i32>(&plugin.store, "pc_filter")
        .unwrap();

    let code = f
        .call(
            &mut plugin.store,
            (
                buf_ptr as i32,
                pixel_bytes,
                8,
                8,
                4,
                format,
                params_ptr as i32,
                params.len() as i32,
            ),
        )
        .unwrap();

    assert_eq!(code, 1, "unsupported colour mode should be an input error");
}

#[test]
fn two_fresh_instances_render_seamless_adjacent_bands() {
    // The core guarantee of the banded model: separate instances, separate
    // memory, no shared state — and the result still joins up.
    let bytes = wasm_bytes();
    let (w, h, ch) = (32u32, 32u32, 4u32);

    // Whole canvas in one instance.
    let whole_band = BandSpec {
        x: 0,
        y: 0,
        width: w,
        height: h,
        channels: ch,
    };
    let whole = {
        let mut plugin = Plugin::instantiate(&bytes);
        plugin
            .run_band(whole_band, &whole_band.params(w, h))
            .unwrap()
    };

    // Same canvas as two bands, each in its own instance.
    let top = BandSpec {
        x: 0,
        y: 0,
        width: w,
        height: h / 2,
        channels: ch,
    };
    let bottom = BandSpec {
        x: 0,
        y: h / 2,
        width: w,
        height: h / 2,
        channels: ch,
    };

    let top_samples = {
        let mut plugin = Plugin::instantiate(&bytes);
        plugin.run_band(top, &top.params(w, h)).unwrap()
    };
    let bottom_samples = {
        let mut plugin = Plugin::instantiate(&bytes);
        plugin.run_band(bottom, &bottom.params(w, h)).unwrap()
    };

    let stride = (w * ch) as usize;
    assert_eq!(top_samples, whole[..stride * (h / 2) as usize]);
    assert_eq!(bottom_samples, whole[stride * (h / 2) as usize..]);
}

#[test]
fn output_is_identical_across_runs() {
    // Determinism through the real ABI, not just the core.
    let bytes = wasm_bytes();
    let band = BandSpec {
        x: 0,
        y: 0,
        width: 16,
        height: 16,
        channels: 4,
    };

    let first = {
        let mut plugin = Plugin::instantiate(&bytes);
        plugin.run_band(band, &band.params(16, 16)).unwrap()
    };
    let second = {
        let mut plugin = Plugin::instantiate(&bytes);
        plugin.run_band(band, &band.params(16, 16)).unwrap()
    };

    assert_eq!(first, second);
}
