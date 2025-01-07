use std::io::Read;

use serde::{Deserialize, Serialize};

/// Minimum number of players required to play cambio
pub const MIN_PLAYER_COUNT: usize = 2;
/// Maximum number of players able to play cambio
pub const MAX_PLAYER_COUNT: usize = 8;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "defaults::snap_time")]
    pub snap_time_secs: u64,
    #[serde(default = "defaults::new_round")]
    pub new_round_timer_secs: u64,
    #[serde(default = "defaults::show_all_cooldown")]
    pub show_all_cooldown: u64,
    #[serde(default = "defaults::port")]
    pub server_port: u16,
}

pub mod defaults {
    pub const fn snap_time() -> u64 {
        5
    }

    pub const fn new_round() -> u64 {
        60
    }

    pub const fn show_all_cooldown() -> u64 {
        1
    }

    pub const fn port() -> u16 {
        25580
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            snap_time_secs: defaults::snap_time(),
            new_round_timer_secs: defaults::new_round(),
            show_all_cooldown: defaults::show_all_cooldown(),
            server_port: defaults::port(),
        }
    }
}

const DEFAULT_CONFIG_PATH: &str = "./Server.toml";

pub fn load() -> anyhow::Result<Config> {
    let mut file = std::fs::File::options()
        .read(true)
        .open(DEFAULT_CONFIG_PATH)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config = toml::from_str(&contents)?;

    Ok(config)
}
