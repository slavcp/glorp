use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::*;
use std::io::*;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use windows::core::*;

#[derive(Deserialize)]
struct SettingInfo {
    #[serde(default)]
    #[serde(rename = "defaultValue")]
    default_value: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    data: HashMap<String, bool>,
}

impl Config {
    pub fn load() -> Config {
        fn load_defaults() -> HashMap<String, bool> {
            let defaults_json = include_str!("./frontend/cSettings.json");
            let settings_info: HashMap<String, SettingInfo> =
                serde_json::from_str(defaults_json).unwrap_or_else(|_| HashMap::new());

            settings_info
                .iter()
                .map(|(key, info)| (key.clone(), info.default_value))
                .collect()
        }

        // w/e im doing it here
        let client_dir: String = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp";
        let swap_dir = String::from(&client_dir) + "\\swapper";
        let scripts_dir = String::from(&client_dir) + "\\scripts";
        let blocklist_path = String::from(&client_dir) + "\\blocklist.json";

        std::fs::create_dir_all(&swap_dir).unwrap_or_default();
        std::fs::create_dir(&scripts_dir).unwrap_or_default();

        if !std::path::Path::new(&blocklist_path).exists() {
            std::fs::write(&blocklist_path, super::constants::DEFAULT_BLOCKLIST)
                .unwrap_or_default();
        }

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
                .write_all(
                    serde_json::to_string_pretty(&load_defaults())
                        .unwrap()
                        .as_bytes(),
                )
                .unwrap();
        }

        let mut settings_string = String::new();
        settings_file.seek(SeekFrom::Start(0)).unwrap();
        settings_file.read_to_string(&mut settings_string).unwrap();

        match serde_json::from_str(&settings_string) {
            Ok(data) => Config { data },
            Err(e) => {
                println!("Error: {}", e);
                Config {
                    data: load_defaults(),
                }
            }
        }
    }

    pub fn get(&self, setting: &str) -> bool {
        self.data.get(setting).copied().unwrap_or(false)
    }

    pub fn set(&mut self, setting: &str, value: bool) {
        self.data.insert(setting.to_string(), value);
    }

    pub unsafe fn send_config(&self, webview_window: &ICoreWebView2) {
        unsafe {
            let config_json = serde_json::to_string_pretty(&self.data).unwrap();
            webview_window
                .PostWebMessageAsJson(PCWSTR(
                    super::utils::create_utf_string(&config_json).as_ptr(),
                ))
                .ok();
        }
    }

    pub fn save(&self) {
        let settings_path =
            std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\settings.json";
        let settings_string = serde_json::to_string_pretty(&self.data).unwrap();
        std::fs::write(settings_path, settings_string).unwrap();
    }
}
