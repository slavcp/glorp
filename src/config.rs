use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::*;
use std::io::*;
#[derive(Deserialize)]
struct SettingInfo {
    #[serde(default)]
    #[serde(rename = "defaultValue")]
    default_value: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    data: HashMap<String, Value>,
}

impl Config {
    pub fn load() -> Config {
        fn load_defaults() -> HashMap<String, Value> {
            let defaults_json = include_str!("./cSettings.json");
            let settings_info: HashMap<String, SettingInfo> =
                serde_json::from_str(defaults_json).unwrap_or_else(|_| HashMap::new());

            settings_info
                .iter()
                .map(|(key, info)| (key.clone(), info.default_value.clone()))
                .collect()
        }
        let client_dir: String = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp";
        let settings_path: String = client_dir + "\\settings.json";

        // recursively create dir
        let mut settings_file = OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .truncate(false)
            .open(&settings_path)
            .unwrap();

        if settings_file.metadata().unwrap().len() == 0 {
            settings_file
                .write_all(serde_json::to_string_pretty(&load_defaults()).unwrap().as_bytes())
                .ok();
        }

        let mut settings_string = String::new();
        settings_file.seek(SeekFrom::Start(0)).ok();
        settings_file.read_to_string(&mut settings_string).ok();

        let mut data = match serde_json::from_str(&settings_string) {
            Ok(data) => data,
            Err(e) => {
                println!("Error: {}", e);
                load_defaults()
            }
        };

        // check for new entries
        let defaults = load_defaults();
        for (key, default_value) in defaults {
            data.entry(key).or_insert(default_value);
        }

        Config { data }
    }

    pub fn get<T: serde::de::DeserializeOwned>(&self, setting: &str) -> Option<T> {
        self.data
            .get(setting)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn set<T: serde::Serialize>(&mut self, setting: &str, value: T) {
        if let Ok(value) = serde_json::to_value(value) {
            self.data.insert(setting.to_string(), value);
        }
    }

    pub fn save(&self) {
        let settings_path = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\settings.json";
        let settings_string = serde_json::to_string_pretty(&self.data).unwrap();
        std::fs::write(settings_path, settings_string).ok();
    }
}
