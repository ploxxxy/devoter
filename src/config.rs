use std::{env, fs};

use serde::Deserialize;

use crate::VoteError;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub votifier_host: String,
    pub votifier_port: u16,
    pub votifier_key: String,
    pub site_name: String,
    pub rate: u64,
    pub max_connections: usize,
}

pub fn load_config() -> Result<Config, VoteError> {
    let mut path = env::current_dir()?;
    path.push("config.json");

    let config_str = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&config_str)?;

    Ok(config)
}

#[derive(Debug, Deserialize)]
struct ScannedPlayers {
    // version: String,
    // #[serde(rename = "exportedAt")]
    // exported_at: u64,
    players: Vec<String>,
}

pub fn load_usernames() -> Result<Vec<String>, VoteError> {
    let mut path = env::current_dir()?;
    path.push("scanned_players.json");

    let usernames_str = fs::read_to_string(path)?;
    let usernames: ScannedPlayers = serde_json::from_str(&usernames_str)?;

    Ok(usernames.players)
}
