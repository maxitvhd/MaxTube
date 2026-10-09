use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::config::Config;
use crate::youtube::Video;

const VIDEOS_FILE: &str = "videos.json";
const PROPOSALS_FILE: &str = "proposals.json";

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("nao consegui ler {}", path.display()))?;
    let val = serde_json::from_str(&raw)
        .with_context(|| format!("{} corrompido (JSON invalido)", path.display()))?;
    Ok(Some(val))
}

fn write_json<T: Serialize>(path: &std::path::Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).ok();
        }
    }
    let raw = serde_json::to_string_pretty(value)?;
    fs::write(path, raw).with_context(|| format!("nao consegui salvar {}", path.display()))?;
    Ok(())
}

pub fn load_videos(cfg: &Config) -> Result<Vec<Video>> {
    Ok(read_json(&cfg.data_path(VIDEOS_FILE))?.unwrap_or_default())
}

pub fn save_videos(cfg: &Config, videos: &[Video]) -> Result<()> {
    write_json(&cfg.data_path(VIDEOS_FILE), &videos)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Approved,
    Rejected,
    Applied,
}

impl Default for Status {
    fn default() -> Self {
        Status::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub video_id: String,
    pub original_title: String,
    pub proposed_title: String,
    #[serde(default)]
    pub status: Status,
}

pub fn load_proposals(cfg: &Config) -> Result<Vec<Proposal>> {
    Ok(read_json(&cfg.data_path(PROPOSALS_FILE))?.unwrap_or_default())
}

pub fn save_proposals(cfg: &Config, proposals: &[Proposal]) -> Result<()> {
    write_json(&cfg.data_path(PROPOSALS_FILE), &proposals)
}
