use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use comfy_table::{Attribute, Cell, Color, Table, presets::UTF8_BORDERS_ONLY};

use crate::types::DependencyRecord;

pub fn print_table(
    records: &[DependencyRecord],
    cwd: &Path,
    search_paths: &[PathBuf],
    home_dir: Option<&Path>,
    hide_source: bool,
) -> Result<()> {
    if records.is_empty() {
        println!("依存関係は見つかりませんでした。");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    let mut header = vec![
        Cell::new("Manager").add_attribute(Attribute::Bold),
        Cell::new("Name").add_attribute(Attribute::Bold),
        Cell::new("Version").add_attribute(Attribute::Bold),
        Cell::new("License").add_attribute(Attribute::Bold),
        Cell::new("Homepage").add_attribute(Attribute::Bold),
    ];
    if !hide_source {
        header.push(Cell::new("Source").add_attribute(Attribute::Bold));
    }
    table.set_header(header);

    for record in records {
        let manager_cell = colorize_manager(&record.manager);
        let license_cell = colorize_license(&record.license);
        let version_cell = Cell::new(record.version.clone().unwrap_or_else(|| "-".to_string()));

        let mut row = vec![
            manager_cell,
            Cell::new(record.name.clone()),
            version_cell,
            license_cell,
            homepage_cell(&record.homepage),
        ];

        if !hide_source {
            let display_source = shorten_source_path(&record.source, cwd, search_paths, home_dir);
            row.push(Cell::new(display_source));
        }

        table.add_row(row);
    }

    println!("{table}");
    Ok(())
}

fn shorten_source_path(
    source: &Path,
    cwd: &Path,
    search_paths: &[PathBuf],
    home_dir: Option<&Path>,
) -> String {
    if let Some(rel) = strip_relative(source, cwd) {
        return rel;
    }

    for base in search_paths {
        if let Some(rel) = source.strip_prefix(base).ok() {
            let rel_text = rel.display().to_string();
            if let Some(name) = base.file_name().map(|n| n.to_string_lossy()) {
                if rel_text.is_empty() {
                    return name.into_owned();
                }
                return format!("{}/{}", name, rel_text);
            } else if !rel_text.is_empty() {
                return rel_text;
            }
        }
    }

    if let Some(home) = home_dir {
        if let Some(rel) = strip_relative(source, home) {
            if rel.is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rel);
        }
    }

    source.display().to_string()
}

fn strip_relative(source: &Path, base: &Path) -> Option<String> {
    source
        .strip_prefix(base)
        .ok()
        .map(|p| p.display().to_string())
}

fn colorize_manager(manager: &str) -> Cell {
    match manager {
        "pip" => Cell::new(manager)
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
        "npm" => Cell::new(manager)
            .fg(Color::Green)
            .add_attribute(Attribute::Bold),
        _ => Cell::new(manager).fg(Color::White),
    }
}

fn colorize_license(license: &str) -> Cell {
    let lower = license.to_ascii_lowercase();

    if lower.contains("gpl") {
        return Cell::new(license)
            .fg(Color::Red)
            .add_attribute(Attribute::Bold);
    }

    if lower.contains("mit") {
        return Cell::new(license)
            .fg(Color::Green)
            .add_attribute(Attribute::Bold);
    }

    if lower.contains("bsd") {
        return Cell::new(license)
            .fg(Color::Blue)
            .add_attribute(Attribute::Bold);
    }

    if lower.contains("apache") {
        return Cell::new(license)
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold);
    }

    if lower == "unknown" {
        return Cell::new(license)
            .fg(Color::Yellow)
            .add_attribute(Attribute::Bold);
    }

    Cell::new(license).fg(Color::Magenta)
}

fn homepage_cell(homepage: &Option<String>) -> Cell {
    match homepage {
        Some(url) => Cell::new(shorten_url(url)),
        None => Cell::new("-"),
    }
}

fn shorten_url(url: &str) -> String {
    const MAX_CHARS: usize = 60;
    let mut buf = String::new();
    for (idx, ch) in url.chars().enumerate() {
        if idx >= MAX_CHARS {
            buf.push_str("...");
            return buf;
        }
        buf.push(ch);
    }
    buf
}

pub fn output_json(
    records: &[DependencyRecord],
    print_json: bool,
    output_path: Option<&Path>,
) -> Result<()> {
    if !print_json && output_path.is_none() {
        return Ok(());
    }

    let json = serde_json::to_string_pretty(records)?;
    if let Some(path) = output_path {
        fs::write(path, &json)
            .with_context(|| format!("JSONファイルの書き込みに失敗: {}", path.display()))?;
        println!("JSONを{}に書き出しました。", path.display());
    }

    if print_json {
        println!("JSON出力:\n{json}");
    }
    Ok(())
}
