use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone)]
pub struct DependencyRecord {
    pub manager: String,
    pub name: String,
    pub version: Option<String>,
    pub license: String,
    pub source: PathBuf,
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub license: Option<String>,
    pub homepage: Option<String>,
}
