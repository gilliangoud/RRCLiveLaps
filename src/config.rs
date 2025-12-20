use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum AppMode {
    Tcp {
        host: String,
        port: u16,
    },
    Usb {
        port_path: String,
    },
    TcpServer {
        port: u16,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub mode: AppMode,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: AppMode::Tcp {
                host: "127.0.0.1".to_string(),
                port: 3601,
            },
        }
    }
}

pub fn load_config(path: &str) -> Config {
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(config) = serde_json::from_str(&content) {
            return config;
        }
    }
    let default_config = Config::default();
    save_config(path, &default_config);
    default_config
}

pub fn save_config(path: &str, config: &Config) {
    if let Ok(json) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, json);
    }
}
