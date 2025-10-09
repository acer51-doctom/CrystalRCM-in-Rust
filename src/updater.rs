use reqwest::Client;
use serde::Deserialize;
use std::error::Error;
use tokio::task::spawn;
use regex::Regex;

use crate::UpdateMessage;

const REPO: &str = "acer51-doctom/CrystalRCM-in-Rust";
const PROPERTIES_PATH: &str = "assets/properties.json";

#[derive(Debug, Deserialize)]
pub struct Properties {
    pub version: String,
    pub update_url: Option<String>,
}

// --- Async version for GUI ---
pub async fn check_for_updates_async(
    repo_url: &str,
    current_version: &str,
    tx: std::sync::mpsc::Sender<UpdateMessage>,
) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let url = format!("https://api.github.com/repos/{}/tags", repo_url);

    let resp = client
        .get(&url)
        .header("User-Agent", "CrystalRCM-Updater")
        .send()
        .await?;

    let tags: Vec<Tag> = resp.json().await?;

    // Filter tags: only vX.Y.Z
    let re = Regex::new(r"^v\d+\.\d+\.\d+$")?;
    let mut valid_tags: Vec<&String> = tags
        .iter()
        .map(|t| &t.name)
        .filter(|name| re.is_match(name))
        .collect();

    valid_tags.sort_by(|a, b| b.cmp(a)); // descending

    if let Some(latest) = valid_tags.first() {
        let latest_ver = latest.trim_start_matches('v');
        if latest_ver != current_version {
            let _ = tx.send(UpdateMessage::Log(format!(
                "⚠️ Update available! Current: {}, Latest: {}",
                current_version, latest_ver
            )));
        } else {
            let _ = tx.send(UpdateMessage::Log("✅ You are running the latest version.".to_string()));
        }
    } else {
        let _ = tx.send(UpdateMessage::Log("⚠️ No valid release tags found.".to_string()));
    }

    Ok(())
}

// --- Sync fallback ---
pub fn check_for_updates() -> Result<(), Box<dyn Error>> {
    let content = std::fs::read_to_string(PROPERTIES_PATH)?;
    let props: Properties = serde_json::from_str(&content)?;
    let current_version = props.version;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(check_for_updates_async(REPO, &current_version, std::sync::mpsc::channel().0))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Tag {
    name: String,
}
