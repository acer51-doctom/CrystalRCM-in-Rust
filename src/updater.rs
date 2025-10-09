use reqwest::blocking::Client;
use serde::Deserialize;
use std::error::Error;
use std::fs;

const REPO: &str = "acer51-doctom/CrystalRCM-in-Rust";
const PROPERTIES_PATH: &str = "assets/properties.json";

#[derive(Debug, Deserialize)]
pub struct Properties {
    pub version: String,
    pub update_url: Option<String>, // optional, you can store latest.json URL
}

pub fn check_for_updates() -> Result<(), Box<dyn Error>> {
    // 1. Load current version from properties.json
    let content = fs::read_to_string(PROPERTIES_PATH)?;
    let props: Properties = serde_json::from_str(&content)?;
    let current_version = props.version;

    // 2. Get all tags from GitHub
    let client = Client::new();
    let url = format!("https://api.github.com/repos/{}/tags", REPO);
    let tags: Vec<Tag> = client
        .get(&url)
        .header("User-Agent", "CrystalRCM-Updater")
        .send()?
        .json()?;

    // 3. Filter only valid release tags
    let mut valid_tags: Vec<&String> = tags.iter()
        .map(|t| &t.name)
        .filter(|name| {
            let re = regex::Regex::new(r"^v\d+\.\d+\.\d+$").unwrap();
            re.is_match(name)
        })
        .collect();

    valid_tags.sort_by(|a, b| b.cmp(a)); // descending

    if let Some(latest) = valid_tags.first() {
        let latest_ver = latest.trim_start_matches('v');
        if latest_ver != current_version {
            println!("⚠️ Update available! Current: {}, Latest: {}", current_version, latest_ver);
        } else {
            println!("✅ You are running the latest version.");
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct Tag {
    name: String,
}
