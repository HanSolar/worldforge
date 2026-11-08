//! Inspects the built `.wasm` module against the PhotoCraft ABI v1 contract.
//!
//! Spec Task 1 acceptance is "a minimal test plugin installs and runs"; the
//! plugin cannot install at all if it declares imports, so this checks the
//! module's import, export and memory sections directly. Parsed by hand to avoid
//! a dependency on an external wasm toolkit being present.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Reads a LEB128 `u32` from `bytes` at `*pos`.
fn read_u32(bytes: &[u8], pos: &mut usize) -> u32 {
    let mut result: u32 = 0;
    let mut shift = 0;
    loop {
        let byte = bytes[*pos];
        *pos += 1;
        result |= u32::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        assert!(shift < 32, "u32 LEB128 overflow");
    }
    result
}

/// Reads a name: length-prefixed UTF-8.
fn read_name(bytes: &[u8], pos: &mut usize) -> String {
    let len = read_u32(bytes, pos) as usize;
    let name = String::from_utf8_lossy(&bytes[*pos..*pos + len]).into_owned();
    *pos += len;
    name
}

fn skip_limits(bytes: &[u8], pos: &mut usize) {
    let flags = bytes[*pos];
    *pos += 1;
    read_u32(bytes, pos);
    if flags & 0x01 != 0 {
        read_u32(bytes, pos);
    }
}

/// A parsed summary of the module.
#[derive(Default)]
struct Module {
    imports: Vec<(String, String, String)>,
    exports: Vec<(String, u8)>,
    memories: usize,
    functions: usize,
}

fn parse(bytes: &[u8]) -> Module {
    assert_eq!(&bytes[..4], b"\0asm", "not a wasm module");
    assert_eq!(&bytes[4..8], &[1, 0, 0, 0], "unexpected wasm version");

    let mut module = Module::default();
    let mut pos = 8;

    while pos < bytes.len() {
        let section_id = bytes[pos];
        pos += 1;
        let section_len = read_u32(bytes, &mut pos) as usize;
        let section_end = pos + section_len;

        match section_id {
            // Import section
            2 => {
                let count = read_u32(bytes, &mut pos);
                for _ in 0..count {
                    let module_name = read_name(bytes, &mut pos);
                    let field = read_name(bytes, &mut pos);
                    let kind = bytes[pos];
                    pos += 1;
                    let desc = match kind {
                        0 => {
                            read_u32(bytes, &mut pos);
                            "func".to_string()
                        }
                        1 => {
                            pos += 1;
                            skip_limits(bytes, &mut pos);
                            "table".to_string()
                        }
                        2 => {
                            skip_limits(bytes, &mut pos);
                            "memory".to_string()
                        }
                        3 => {
                            pos += 2;
                            "global".to_string()
                        }
                        _ => panic!("unknown import kind {kind}"),
                    };
                    module.imports.push((module_name, field, desc));
                }
            }
            // Function section
            3 => {
                module.functions = read_u32(bytes, &mut pos) as usize;
            }
            // Memory section
            5 => {
                let count = read_u32(bytes, &mut pos);
                module.memories = count as usize;
                for _ in 0..count {
                    skip_limits(bytes, &mut pos);
                }
            }
            // Export section
            7 => {
                let count = read_u32(bytes, &mut pos);
                for _ in 0..count {
                    let name = read_name(bytes, &mut pos);
                    let kind = bytes[pos];
                    pos += 1;
                    let _index = read_u32(bytes, &mut pos);
                    module.exports.push((name, kind));
                }
            }
            _ => {}
        }

        pos = section_end;
    }

    module
}

fn wasm_path() -> PathBuf {
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
            .expect("spawn cargo build for wasm verification");
        assert!(status.success(), "failed to build wasm artifact");
        path
    })
    .clone()
}

#[test]
fn module_builds_and_parses() {
    let path = wasm_path();
    assert!(path.exists(), "missing: {}", path.display());

    let bytes = std::fs::read(&path).expect("read wasm");
    let module = parse(&bytes);

    assert!(module.functions > 0, "module has no functions");
}

#[test]
fn module_declares_no_imports() {
    // The single most important contract: "Host imports: none ... module rejected
    // at install". No WASI, no clock, no randomness.
    let bytes = std::fs::read(wasm_path()).expect("read wasm");
    let module = parse(&bytes);

    assert!(
        module.imports.is_empty(),
        "module declares {} import(s): {:?}",
        module.imports.len(),
        module.imports
    );
}

#[test]
fn module_exports_the_abi_surface() {
    let bytes = std::fs::read(wasm_path()).expect("read wasm");
    let module = parse(&bytes);

    let names: BTreeSet<&str> = module.exports.iter().map(|(n, _)| n.as_str()).collect();

    for required in [
        "memory",
        "pc_abi_version",
        "pc_manifest",
        "pc_alloc",
        "pc_filter",
    ] {
        assert!(
            names.contains(required),
            "missing export `{required}`; found {names:?}"
        );
    }
}

#[test]
fn module_has_exactly_one_memory() {
    let bytes = std::fs::read(wasm_path()).expect("read wasm");
    let module = parse(&bytes);

    assert_eq!(
        module.memories, 1,
        "the ABI expects a single exported memory"
    );
}

#[test]
fn module_size_is_within_the_host_limit() {
    let bytes = std::fs::read(wasm_path()).expect("read wasm");
    // Host rejects modules over 32 MiB.
    assert!(
        bytes.len() < 32 * 1024 * 1024,
        "module is {} bytes, over the 32 MiB limit",
        bytes.len()
    );
}
