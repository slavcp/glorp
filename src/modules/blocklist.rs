use std::{collections::HashSet, fs::*, io::*};

use regex::Regex;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;

use crate::constants;
use crate::utils;

#[derive(serde::Deserialize, serde::Serialize)]
struct BlocklistConfig {
    enabled: HashSet<String>,
    disabled: HashSet<String>,
}

pub fn load(webview_window: &ICoreWebView2_22) -> Vec<Regex> {
    let blocklist_path: String = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\blocklist.json";
    let mut blocklist_file = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(false)
        .open(&blocklist_path)
        .unwrap();

    if blocklist_file.metadata().unwrap().len() == 0 {
        blocklist_file
            .write_all(constants::DEFAULT_BLOCKLIST.as_bytes())
            .unwrap();
    }

    let blocklist_string = std::fs::read_to_string(&blocklist_path).unwrap();

    let mut blocklist = match serde_json::from_str::<BlocklistConfig>(&blocklist_string) {
        Ok(config) => config,
        Err(_) => {
            blocklist_file
                .write_all(constants::DEFAULT_BLOCKLIST.as_bytes())
                .unwrap();
            serde_json::from_str::<BlocklistConfig>(constants::DEFAULT_BLOCKLIST).unwrap()
        }
    };

    let default_urls = serde_json::from_str::<BlocklistConfig>(constants::DEFAULT_BLOCKLIST).unwrap();

    for url in default_urls.disabled {
        blocklist.disabled.insert(url);
    }

    for url in default_urls.enabled {
        blocklist.enabled.insert(url);
    }

    blocklist.enabled.retain(|url| !blocklist.disabled.contains(url));

    let updated_blocklist_string = serde_json::to_string_pretty(&blocklist).unwrap();
    blocklist_file.set_len(0).unwrap();
    blocklist_file.seek(std::io::SeekFrom::Start(0)).unwrap();
    blocklist_file.write_all(updated_blocklist_string.as_bytes()).unwrap();

    let mut blocklist_regex: Vec<Regex> = Vec::<Regex>::new();

    for url in blocklist.enabled.iter() {
        unsafe {
            if let Err(e) = webview_window.AddWebResourceRequestedFilterWithRequestSourceKinds(
                PCWSTR(utils::create_utf_string(url).as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
            ) {
                eprintln!("Failed to add web resource requested filter: {}", e);
            }
        }
        let pattern = url.replace("*", ".*");
        let pattern = format!("^{}$", pattern);
        let regex = Regex::new(&pattern).unwrap();
        blocklist_regex.push(regex);
    }
    blocklist_regex
}
