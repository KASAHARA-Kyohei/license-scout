mod cache;
mod cli;
mod metadata;
mod output;
mod progress;
mod scan;
mod types;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::cache::LicenseCache;
use crate::cli::Cli;
use crate::types::DependencyRecord;

fn main() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let cli = Cli::parse();
    let search_paths = resolve_search_paths(&cli.paths, &cwd);

    let mut records = progress::with_spinner("依存関係を解析中...", |spinner| {
        let mut acc = Vec::<DependencyRecord>::new();
        for dir in &search_paths {
            spinner.set_message(format!("解析中: {}", dir.display()));
            acc.extend(scan::collect_records(dir)?);
        }
        Ok(acc)
    })?;

    records.sort_by(|a, b| {
        a.manager
            .cmp(&b.manager)
            .then(a.name.cmp(&b.name))
            .then(a.version.cmp(&b.version))
            .then(a.source.cmp(&b.source))
    });

    if cli.fetch_licenses {
        let mut cache = LicenseCache::load()?;
        progress::with_spinner("ライセンス情報を取得中...", |spinner| {
            metadata::enrich_metadata(&mut records, Some(spinner), &mut cache)
        })?;
        cache.save()?;
    }

    if let Some(query) = cli.search.as_deref() {
        let needle = query.to_ascii_lowercase();
        let before = records.len();
        records.retain(|record| record_matches_query(record, &needle));
        println!(
            "> 検索クエリ \"{query}\" を適用: {before}件 -> {}件",
            records.len()
        );
        if records.is_empty() {
            println!("指定の検索条件に一致する依存関係はありません。");
        }
    }

    let home_dir = dirs::home_dir();
    println!("> レポートを出力中...");
    output::print_table(
        &records,
        &cwd,
        &search_paths,
        home_dir.as_deref(),
        cli.hide_source,
    )?;
    output::output_json(&records, cli.print_json, cli.json_output.as_deref())?;
    println!("✔ レポート出力完了");

    Ok(())
}

fn resolve_search_paths(paths: &[PathBuf], cwd: &std::path::Path) -> Vec<PathBuf> {
    if paths.is_empty() {
        vec![cwd.to_path_buf()]
    } else {
        paths
            .iter()
            .map(|p| {
                if p.is_relative() {
                    cwd.join(p)
                } else {
                    p.clone()
                }
            })
            .collect()
    }
}

fn record_matches_query(record: &DependencyRecord, needle: &str) -> bool {
    let version = record.version.as_deref().unwrap_or("");
    let homepage = record.homepage.as_deref().unwrap_or("");
    let source = record.source.display().to_string();

    let targets = [
        record.manager.as_str(),
        record.name.as_str(),
        record.license.as_str(),
        version,
        homepage,
        source.as_str(),
    ];

    targets
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .any(|value| value.contains(needle))
}
