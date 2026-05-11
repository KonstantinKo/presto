// Generates `themes.rs` from `src/style/themes/*.css` into `OUT_DIR` so
// `pub mod themes { include!(...) }` in `src/theme/mod.rs` can pick it up.
// Mirrors the Trunk pre-build hook in `src/Trunk.toml` so that plain
// `cargo build` and `cargo test` work without invoking Trunk first.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR set by cargo");

    let themes_dir = Path::new(&manifest_dir).join("style").join("themes");
    let out_path = Path::new(&out_dir).join("themes.rs");

    // Rerun only when a CSS file is added, removed, or renamed in themes/.
    // The individual file contents don't affect the catalogue — only stems.
    println!("cargo:rerun-if-changed={}", themes_dir.display());

    let mut stems: Vec<String> = Vec::new();
    if themes_dir.is_dir() {
        for entry in fs::read_dir(&themes_dir).expect("read themes dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if file_name.starts_with('.') {
                continue;
            }
            let Some(stem) = file_name.strip_suffix(".css") else {
                continue;
            };
            if stem.is_empty() {
                continue;
            }
            stems.push(stem.to_owned());
        }
    }
    stems.sort();
    stems.dedup();

    let mut out = String::new();
    out.push_str("// @generated — DO NOT EDIT. Re-run `cargo build` to regenerate.\n");
    out.push_str("// Source: src/style/themes/*.css via src/build.rs\n\n");
    out.push_str("/// Alphabetised list of every theme stem.\n");
    out.push_str("#[rustfmt::skip]\n");
    out.push_str("pub const ALL_THEMES: &[&str] = &[");
    if !stems.is_empty() {
        out.push('\n');
        for stem in &stems {
            out.push_str("    \"");
            out.push_str(stem);
            out.push_str("\",\n");
        }
    }
    out.push_str("];\n\n");
    out.push_str("/// Default theme applied at first launch (first alphabetically).\n");
    out.push_str("pub const DEFAULT_THEME: &str = \"");
    if let Some(first) = stems.first() {
        out.push_str(first);
    }
    out.push_str("\";\n");

    fs::write(&out_path, &out).expect("write themes.rs to OUT_DIR");
}
