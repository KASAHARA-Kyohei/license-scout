use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;
use walkdir::WalkDir;

use crate::types::DependencyRecord;

pub fn collect_records(root: &Path) -> Result<Vec<DependencyRecord>> {
    if !root.exists() {
        bail!("指定されたパスが存在しません: {}", root.display());
    }

    let mut collected = Vec::new();
    let walker = WalkDir::new(root).into_iter().filter_entry(|entry| {
        if entry.depth() == 0 {
            return true;
        }
        let name = entry
            .file_name()
            .to_str()
            .unwrap_or_default()
            .to_ascii_lowercase();
        entry.depth() <= 64
            && !matches!(
                name.as_str(),
                "node_modules" | ".git" | "target" | "__pycache__" | "venv" | ".venv"
            )
    });

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("警告: ディレクトリの走査に失敗しました: {err}");
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        match entry.file_name().to_string_lossy().as_ref() {
            "requirements.txt" => {
                collected.extend(parse_requirements(entry.path()).with_context(|| {
                    format!("requirements.txtの解析に失敗: {}", entry.path().display())
                })?);
            }
            "package-lock.json" => {
                collected.extend(parse_package_lock(entry.path()).with_context(|| {
                    format!("package-lock.jsonの解析に失敗: {}", entry.path().display())
                })?);
            }
            _ => {}
        }
    }

    Ok(collected)
}

fn parse_requirements(path: &Path) -> Result<Vec<DependencyRecord>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("requirements.txtの読み込みに失敗: {}", path.display()))?;

    let mut records = Vec::new();
    for line in content.lines() {
        if let Some((name, version)) = parse_requirement_line(line) {
            records.push(DependencyRecord {
                manager: "pip".to_string(),
                name,
                version,
                license: "Unknown".to_string(),
                source: path.to_path_buf(),
                homepage: None,
            });
        }
    }

    Ok(records)
}

fn parse_requirement_line(line: &str) -> Option<(String, Option<String>)> {
    let without_comment = line.split('#').next()?.trim();
    if without_comment.is_empty() || without_comment.starts_with('-') {
        return None;
    }

    let requirement = without_comment.split(';').next()?.trim();
    if requirement.is_empty() {
        return None;
    }

    let markers: &[&str] = &["===", "==", ">=", "<=", "~=", "!=", ">", "<", "="];
    for marker in markers {
        if let Some(idx) = requirement.find(marker) {
            let (name_part, version_part) = requirement.split_at(idx);
            let version = version_part[marker.len()..].trim();
            return Some((
                normalize_package_name(name_part.trim())?,
                (!version.is_empty()).then(|| version.to_string()),
            ));
        }
    }

    Some((normalize_package_name(requirement)?, None))
}

fn normalize_package_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        let base = trimmed
            .split('[')
            .next()
            .unwrap_or(trimmed)
            .replace('_', "-");
        Some(base)
    }
}

fn parse_package_lock(path: &Path) -> Result<Vec<DependencyRecord>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("package-lock.jsonの読み込みに失敗: {}", path.display()))?;
    let json: Value = serde_json::from_str(&text)
        .with_context(|| format!("package-lock.jsonのJSON解析に失敗: {}", path.display()))?;

    if let Some(packages) = json.get("packages").and_then(|v| v.as_object()) {
        Ok(packages
            .iter()
            .filter_map(|(pkg_path, info)| build_package_lock_record(pkg_path, info, path, &json))
            .collect())
    } else if let Some(deps) = json.get("dependencies").and_then(|v| v.as_object()) {
        let mut records = Vec::new();
        collect_from_dependencies_map(deps, path, &mut records);
        Ok(records)
    } else {
        Ok(Vec::new())
    }
}

fn build_package_lock_record(
    pkg_path: &str,
    info: &Value,
    source: &Path,
    root_json: &Value,
) -> Option<DependencyRecord> {
    let version = info
        .get("version")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let license = info
        .get("license")
        .and_then(extract_license)
        .unwrap_or_else(|| "Unknown".to_string());
    let name = info
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| package_name_from_path(pkg_path))
        .or_else(|| {
            root_json
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })?;

    Some(DependencyRecord {
        manager: "npm".to_string(),
        name,
        version,
        license,
        source: source.to_path_buf(),
        homepage: None,
    })
}

fn collect_from_dependencies_map(
    map: &serde_json::Map<String, Value>,
    source: &Path,
    acc: &mut Vec<DependencyRecord>,
) {
    for (name, value) in map {
        let version = value
            .get("version")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        acc.push(DependencyRecord {
            manager: "npm".to_string(),
            name: name.clone(),
            version,
            license: value
                .get("license")
                .and_then(extract_license)
                .unwrap_or_else(|| "Unknown".to_string()),
            source: source.to_path_buf(),
            homepage: None,
        });
        if let Some(inner) = value.get("dependencies").and_then(|v| v.as_object()) {
            collect_from_dependencies_map(inner, source, acc);
        }
    }
}

fn package_name_from_path(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }

    let mut segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return None;
    }

    if let Some(pos) = segments
        .iter()
        .rposition(|segment| *segment == "node_modules")
    {
        segments = segments.split_off(pos + 1);
    }

    if segments.is_empty() {
        None
    } else if segments[0].starts_with('@') && segments.len() >= 2 {
        Some(format!("{}/{}", segments[0], segments[1]))
    } else {
        Some(segments[0].to_string())
    }
}

pub fn extract_license(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.to_string()),
        Value::Array(values) => {
            let merged: Vec<String> = values.iter().filter_map(extract_license).collect();
            (!merged.is_empty()).then(|| merged.join(", "))
        }
        Value::Object(map) => map
            .get("type")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirement_line_parsing() {
        assert_eq!(
            parse_requirement_line("requests==2.32.0"),
            Some(("requests".to_string(), Some("2.32.0".to_string())))
        );
        assert_eq!(
            parse_requirement_line("uvicorn[standard]>=0.27"),
            Some(("uvicorn".to_string(), Some("0.27".to_string())))
        );
        assert_eq!(parse_requirement_line("# comment line"), None);
        assert_eq!(parse_requirement_line(""), None);
    }

    #[test]
    fn package_name_from_path_handles_scoped_packages() {
        assert_eq!(
            package_name_from_path("node_modules/@types/node"),
            Some("@types/node".to_string())
        );
        assert_eq!(
            package_name_from_path("node_modules/lodash"),
            Some("lodash".to_string())
        );
    }
}
