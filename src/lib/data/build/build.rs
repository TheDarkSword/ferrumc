use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::fs;
use std::io::Write;

mod attributes;
mod biomes;
mod blocks;
mod damage_types;
mod default_attributes;
mod effects;
mod enchantments;
mod fluids;
mod items;
mod particles;
mod potions;
mod sounds;

pub const OUT_DIR: &str = "src/generated";

pub fn main() {
    println!("Running build.rs...");

    // Write a test file to verify build script runs
    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| "test".to_string());
    let test_file = std::path::Path::new(&out_dir).join("build_test.txt");
    std::fs::write(&test_file, "Build script ran!").unwrap();
    println!("Wrote test file to: {:?}", test_file);

    let path = std::path::Path::new(OUT_DIR);
    if !path.exists() {
        let _ = fs::create_dir(OUT_DIR);
    }

    // Build functions in order of dependency (excluding blocks)
    #[allow(clippy::type_complexity)]
    let build_functions: Vec<(fn() -> proc_macro2::TokenStream, &str)> = vec![
        (particles::build, "particles.rs"),
        (sounds::build, "sounds.rs"),
        (attributes::build, "attributes.rs"),
        (default_attributes::build, "default_attributes.rs"),
        (damage_types::build, "damage_types.rs"),
        (fluids::build, "fluids.rs"),
        (effects::build, "effects.rs"),
        (enchantments::build, "enchantments.rs"),
        (potions::build, "potions.rs"),
        (biomes::build, "biomes.rs"),
        (items::build, "items.rs"),
    ];

    // Build other files normally
    build_functions.par_iter().for_each(|(build_fn, file)| {
        println!("Building {}...", file);
        let formatted_code = format_code(&build_fn().to_string());
        write_generated_file(&formatted_code, file);
        println!("Finished building {}", file);
    });

    // Build blocks separately (uses OUT_DIR from cargo)
    match blocks::build() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Failed to build blocks: {}", e);
            std::process::exit(1);
        }
    }
}

/// A number written so nothing is lost, and so it is still a float literal.
///
/// Rounding to a fixed number of places is how gravity's 0.08 became 0.1 and a carrot's 3.6000001
/// became 3.6; the same one-liner was copied into five generators.
pub fn number(value: f64) -> syn::LitFloat {
    syn::LitFloat::new(&written(value), proc_macro2::Span::call_site())
}

/// The same for a value the game keeps as a single-precision float.
///
/// Written at single precision on purpose: the report renders such a value at double precision, so
/// a saturation of 2.4 arrives as 2.4000000953674316, and writing that into an `f32` field is the
/// same number said at a length that means nothing.
pub fn number_f32(value: f32) -> syn::LitFloat {
    let written = format!("{value:?}");
    let written = if written.contains(['.', 'e', 'E']) {
        written
    } else {
        format!("{written}.0")
    };
    syn::LitFloat::new(&written, proc_macro2::Span::call_site())
}

/// The same, as text, for a generator that writes source rather than tokens.
pub fn written(value: f64) -> String {
    let written = format!("{value:?}");
    if written.contains(['.', 'e', 'E']) {
        written
    } else {
        format!("{written}.0")
    }
}

pub fn array_to_tokenstream(array: &[String]) -> proc_macro2::TokenStream {
    let mut variants = proc_macro2::TokenStream::new();

    for item in array.iter() {
        let name = quote::format_ident!("{}", heck::ToPascalCase::to_pascal_case(item.as_str()));
        variants.extend([quote::quote! {
            #name,
        }]);
    }
    variants
}

pub fn write_generated_file(new_code: &str, out_file: &str) {
    let path = std::path::Path::new(OUT_DIR).join(out_file);

    if path.exists() {
        if let Ok(existing_code) = fs::read_to_string(&path) {
            if existing_code == new_code {
                return; // No changes, so we skip writing.
            }
        }
    }

    fs::write(&path, new_code)
        .unwrap_or_else(|_| panic!("Failed to write to file: {}", path.display()));
}

pub fn format_code(unformatted_code: &str) -> String {
    let mut child = std::process::Command::new("rustfmt")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn rustfmt process.");

    child
        .stdin
        .take()
        .expect("Failed to take rustfmt stdin")
        .write_all(unformatted_code.as_bytes())
        .expect("Failed to write to rustfmt stdin.");

    let output = child
        .wait_with_output()
        .expect("Failed to wait for rustfmt process.");

    if output.status.success() {
        String::from_utf8(output.stdout).expect("rustfmt output was not valid UTF-8.")
    } else {
        panic!(
            "rustfmt failed with status: {}\n--- stderr ---\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
