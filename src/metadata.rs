use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::{StatusCode, blocking::Client};
use serde::Deserialize;
use serde_json::Value;
use urlencoding::encode;

use indicatif::ProgressBar;

use crate::cache::LicenseCache;
use crate::scan::extract_license;
use crate::types::{DependencyRecord, PackageMetadata};

pub fn enrich_metadata(
    records: &mut [DependencyRecord],
    progress: Option<&ProgressBar>,
    cache: &mut LicenseCache,
) -> Result<()> {
    if records.is_empty() {
        return Ok(());
    }

    let total_targets = records.iter().filter(|r| needs_metadata(r)).count();
    if total_targets == 0 {
        if let Some(pb) = progress {
            pb.set_message("ライセンス情報を取得中... (0/0)");
        }
        return Ok(());
    }

    let client = Client::builder()
        .user_agent("license-scout/0.1.0")
        .timeout(Duration::from_secs(10))
        .build()
        .context("HTTPクライアントの初期化に失敗しました")?;

    let mut session_cache: HashMap<(String, String), Option<PackageMetadata>> = HashMap::new();
    let mut processed = 0usize;

    for record in records.iter_mut() {
        if !needs_metadata(record) {
            continue;
        }

        processed += 1;
        if let Some(pb) = progress {
            pb.set_message(format!(
                "ライセンス情報を取得中... ({processed}/{total_targets})"
            ));
        }

        let key = (record.manager.clone(), record.name.clone());
        if let Some(cached) = session_cache.get(&key) {
            apply_metadata(record, cached);
            continue;
        }

        if let Some(cached) = cache.get(&record.manager, &record.name) {
            apply_metadata(record, &Some(cached.clone()));
            session_cache.insert(key.clone(), Some(cached));
            continue;
        }

        let fetched = match record.manager.as_str() {
            "pip" => fetch_pypi_metadata(&client, &record.name),
            "npm" => fetch_npm_metadata(&client, &record.name, record.version.as_deref()),
            _ => Ok(None),
        };

        match fetched {
            Ok(Some(metadata)) => {
                apply_metadata(record, &Some(metadata.clone()));
                cache.insert(&record.manager, &record.name, metadata.clone());
                session_cache.insert(key, Some(metadata));
            }
            Ok(None) => {
                session_cache.insert(key, None);
            }
            Err(err) => {
                eprintln!(
                    "警告: {}({})のライセンス取得に失敗しました: {err}",
                    record.name, record.manager
                );
                session_cache.insert(key, None);
            }
        }
    }

    Ok(())
}

fn needs_metadata(record: &DependencyRecord) -> bool {
    record.homepage.is_none()
        || record.license.trim().is_empty()
        || record.license.eq_ignore_ascii_case("unknown")
}

fn apply_metadata(record: &mut DependencyRecord, metadata: &Option<PackageMetadata>) {
    if let Some(meta) = metadata {
        if should_update_license(&record.license, meta.license.as_deref()) {
            if let Some(license) = &meta.license {
                record.license = license.clone();
            }
        }
        if record.homepage.is_none() {
            record.homepage = meta.homepage.clone();
        }
    }
}

fn should_update_license(current: &str, candidate: Option<&str>) -> bool {
    candidate.is_some() && (current.trim().is_empty() || current.eq_ignore_ascii_case("unknown"))
}

#[derive(Debug, Deserialize)]
struct PyPiResponse {
    info: PyPiInfo,
}

#[derive(Debug, Deserialize)]
struct PyPiInfo {
    license: Option<String>,
    classifiers: Option<Vec<String>>,
    #[serde(rename = "home_page")]
    home_page: Option<String>,
    #[serde(rename = "project_urls")]
    project_urls: Option<HashMap<String, String>>,
}

fn fetch_pypi_metadata(client: &Client, package_name: &str) -> Result<Option<PackageMetadata>> {
    let encoded = encode(package_name);
    let url = format!("https://pypi.org/pypi/{encoded}/json");
    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("PyPIリクエストに失敗しました: {package_name}"))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !response.status().is_success() {
        bail!(
            "PyPIがエラーを返しました({package_name}): {}",
            response.status()
        );
    }

    let data: PyPiResponse = response
        .json()
        .with_context(|| format!("PyPIレスポンスの解析に失敗: {package_name}"))?;

    let license = data
        .info
        .license
        .as_deref()
        .and_then(normalize_license_text)
        .or_else(|| {
            data.info
                .classifiers
                .as_ref()
                .and_then(|c| license_from_classifiers(c))
        });

    let homepage = extract_pypi_homepage(&data.info);

    if license.is_some() || homepage.is_some() {
        Ok(Some(PackageMetadata { license, homepage }))
    } else {
        Ok(None)
    }
}

fn license_from_classifiers(classifiers: &[String]) -> Option<String> {
    classifiers
        .iter()
        .filter_map(|classifier| {
            if classifier.contains("License ::") {
                classifier
                    .split("::")
                    .last()
                    .map(|part| part.trim().to_string())
            } else {
                None
            }
        })
        .find(|value| !value.is_empty())
}

fn extract_pypi_homepage(info: &PyPiInfo) -> Option<String> {
    if let Some(urls) = &info.project_urls {
        for key in [
            "Homepage",
            "Home Page",
            "Source",
            "Repository",
            "Documentation",
        ] {
            if let Some(value) = urls.get(key).and_then(|url| normalize_homepage(url)) {
                return Some(value);
            }
        }

        for value in urls.values() {
            if let Some(url) = normalize_homepage(value) {
                return Some(url);
            }
        }
    }

    info.home_page.as_deref().and_then(normalize_homepage)
}

fn normalize_homepage(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let cleaned = trimmed.trim_end_matches('/');
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn normalize_license_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn fetch_npm_metadata(
    client: &Client,
    package_name: &str,
    version: Option<&str>,
) -> Result<Option<PackageMetadata>> {
    let encoded = encode(package_name);
    let url = format!("https://registry.npmjs.org/{encoded}");
    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("npm Registryリクエストに失敗しました: {package_name}"))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !response.status().is_success() {
        bail!(
            "npm Registryがエラーを返しました({package_name}): {}",
            response.status()
        );
    }

    let data: Value = response
        .json()
        .with_context(|| format!("npmレスポンスの解析に失敗: {package_name}"))?;

    if let Some(ver) = version {
        if let Some(metadata) = lookup_npm_version_metadata(&data, ver) {
            return Ok(Some(metadata));
        }
    }

    let license = data.get("license").and_then(extract_license);
    let homepage = extract_npm_homepage(&data);

    if license.is_some() || homepage.is_some() {
        return Ok(Some(PackageMetadata { license, homepage }));
    }

    if let Some(latest) = data
        .get("dist-tags")
        .and_then(|tags| tags.get("latest"))
        .and_then(|v| v.as_str())
    {
        if let Some(metadata) = lookup_npm_version_metadata(&data, latest) {
            return Ok(Some(metadata));
        }
    }

    Ok(None)
}

fn lookup_npm_version_metadata(json: &Value, version: &str) -> Option<PackageMetadata> {
    let entry = json
        .get("versions")
        .and_then(|versions| versions.get(version))?;
    let license = entry.get("license").and_then(extract_license);
    let homepage = extract_npm_homepage(entry);

    if license.is_none() && homepage.is_none() {
        None
    } else {
        Some(PackageMetadata { license, homepage })
    }
}

fn extract_npm_homepage(value: &Value) -> Option<String> {
    value
        .get("homepage")
        .and_then(|v| v.as_str())
        .and_then(normalize_homepage)
        .or_else(|| value.get("repository").and_then(extract_npm_repository_url))
}

fn extract_npm_repository_url(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => normalize_repository_url(s),
        Value::Object(map) => map
            .get("url")
            .and_then(|v| v.as_str())
            .and_then(normalize_repository_url),
        _ => None,
    }
}

fn normalize_repository_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let cleaned = trimmed.strip_prefix("git+").unwrap_or(trimmed);
    let cleaned = cleaned.trim_end_matches(".git");
    normalize_homepage(cleaned)
}
