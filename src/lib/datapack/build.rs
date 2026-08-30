//! Builds the pack the server ships with.
//!
//! Vanilla keeps its datapack inside the server jar; the same files live in the executable here,
//! as a zip read through the same reader that opens a user's pack. The source is the extracted
//! data under `assets/extracted/`, minified on the way in — indented json is ten times the size
//! for nothing, since it is only ever parsed.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

/// The version the server speaks. Everything version-derived is read from this directory, so a
/// bump moves in one place.
const EXTRACTED: &str = "../../../assets/extracted/26.2";

/// Vanilla exposes these as separate, feature-flagged packs rather than as part of its own.
const NOT_PART_OF_THE_PACK: &str = "minecraft/datapacks";

fn main() {
    let extracted = PathBuf::from(EXTRACTED);
    println!("cargo:rerun-if-changed={EXTRACTED}/data");
    println!("cargo:rerun-if-changed={EXTRACTED}/version.json");

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let (major, minor) = pack_version(&extracted);
    write_pack(
        &extracted.join("data"),
        &out.join("vanilla.zip"),
        major,
        minor,
    );
    fs::write(
        out.join("pack_format.rs"),
        format!(
            "/// The pack format the running game is. Read from the extracted version so it cannot\n\
             /// drift from the data the built-in pack was built out of.\n\
             pub const CURRENT_PACK_FORMAT: PackFormat = PackFormat::new({major}, {minor});\n"
        ),
    )
    .expect("the pack format constant should be writable");
}

/// The pack format the extracted version declares, which is the one the built-in pack is written
/// for and the one every other pack is measured against.
fn pack_version(extracted: &Path) -> (u64, u64) {
    let version = extracted.join("version.json");
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(&version).expect("the extracted version should be there"))
            .expect("version.json should be json");
    let pack = &json["pack_version"];
    let read = |field: &str| {
        pack[field]
            .as_u64()
            .unwrap_or_else(|| panic!("version.json should carry pack_version.{field}"))
    };
    (read("data_major"), read("data_minor"))
}

fn write_pack(data: &Path, zip_path: &Path, major: u64, minor: u64) {
    let file = fs::File::create(zip_path).expect("the pack should be writable");
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9));

    // A bare major in `max_format` means every minor release of it, which is what a pack built for
    // the running game supports.
    let meta = format!(
        r#"{{"pack":{{"description":{{"translate":"dataPack.vanilla.description"}},"min_format":[{major},{minor}],"max_format":[{major}]}}}}"#
    );
    zip.start_file("pack.mcmeta", options)
        .expect("the pack should be writable");
    zip.write_all(meta.as_bytes())
        .expect("the pack should be writable");

    let mut entries = Vec::new();
    collect(data, data, &mut entries);
    // Sorted so two builds of the same input produce the same archive.
    entries.sort();
    for relative in entries {
        let source = data.join(&relative);
        let bytes = fs::read(&source).expect("an extracted file should be readable");
        let bytes = if source.extension().is_some_and(|e| e == "json") {
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| panic!("{} should be json: {e}", source.display()));
            serde_json::to_vec(&value).expect("a parsed value should serialize")
        } else {
            bytes
        };
        zip.start_file(format!("data/{relative}"), options)
            .expect("the pack should be writable");
        zip.write_all(&bytes).expect("the pack should be writable");
    }

    zip.finish().expect("the pack should be writable");
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let listing = fs::read_dir(dir).expect("the extracted data should be readable");
    for entry in listing {
        let path = entry.expect("the extracted data should be readable").path();
        let relative = path
            .strip_prefix(root)
            .expect("walked from the root")
            .to_str()
            .expect("extracted paths are utf-8")
            .replace('\\', "/");
        if relative == NOT_PART_OF_THE_PACK {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
        } else {
            out.push(relative);
        }
    }
}
