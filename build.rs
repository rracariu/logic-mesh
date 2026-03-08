// Copyright (c) 2022-2026, Radu Racariu.

//! Build script that scans `src/blocks/` for structs annotated with `#[block]`
//! and generates the block registry code automatically.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let blocks_dir = Path::new("src/blocks");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir).join("block_registry.rs");

    // Tell Cargo to re-run if any file in src/blocks changes
    println!("cargo::rerun-if-changed=src/blocks");

    // Collect blocks: category -> [(module_name, struct_name)]
    let mut blocks: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();

    // Walk subdirectories of src/blocks/
    let entries = fs::read_dir(blocks_dir).expect("Failed to read src/blocks/");
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let category = path.file_name().unwrap().to_string_lossy().into_owned();

        // Skip non-block directories
        if matches!(category.as_str(), "utils") {
            continue;
        }

        let mut category_blocks = Vec::new();

        let files = fs::read_dir(&path).expect("Failed to read block category dir");
        for file in files.flatten() {
            let file_path = file.path();
            if file_path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }

            let file_name = file_path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();

            // Skip mod.rs
            if file_name == "mod" {
                continue;
            }

            if let Some(struct_name) = find_block_struct(&file_path) {
                category_blocks.push((file_name, struct_name));
            }
        }

        category_blocks.sort_by(|a, b| a.1.cmp(&b.1));

        if !category_blocks.is_empty() {
            blocks.insert(category, category_blocks);
        }
    }

    // Generate the registry code
    let mut code = String::new();

    // Generate use statements
    for (category, category_blocks) in &blocks {
        for (module, struct_name) in category_blocks {
            code.push_str(&format!(
                "use crate::blocks::{category}::{module}::{struct_name};\n"
            ));
        }
    }

    code.push('\n');

    // Generate register_blocks! invocation
    code.push_str("register_blocks!(\n");

    let total_categories = blocks.len();
    for (cat_idx, (category, category_blocks)) in blocks.iter().enumerate() {
        code.push_str(&format!("    // {} blocks\n", capitalize(category)));
        for (i, (_, struct_name)) in category_blocks.iter().enumerate() {
            code.push_str(&format!("    {struct_name}"));
            // Add comma unless it's the very last entry
            if cat_idx < total_categories - 1 || i < category_blocks.len() - 1 {
                code.push(',');
            }
            code.push('\n');
        }
    }

    code.push_str(");\n");

    fs::write(&out_path, code).expect("Failed to write block registry");
}

/// Scan a Rust source file for `#[block]` followed by `pub struct Name`.
fn find_block_struct(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let mut found_block_attr = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "#[block]" {
            found_block_attr = true;
            continue;
        }

        if found_block_attr {
            // Skip other attributes and derive macros between #[block] and pub struct
            if trimmed.starts_with("#[") || trimmed.starts_with("//") {
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("pub struct ") {
                // Extract struct name (until whitespace or '{' or '<')
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();

                if !name.is_empty() {
                    return Some(name);
                }
            }

            // If we hit a non-attribute, non-struct line, reset
            found_block_attr = false;
        }
    }

    None
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
