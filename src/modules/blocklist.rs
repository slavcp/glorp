use std::{collections::HashSet, fs::*, io::*};

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;

use crate::constants;
use crate::utils;

#[derive(serde::Deserialize, serde::Serialize)]
struct UserBlocklist {
    blocked: HashSet<String>,
    disabled_defaults: HashSet<String>,
}

pub fn load(webview_window: &ICoreWebView2) {
    let example_blocklist: &str = r#"
{
    "blocked": [
        "*://example1.com",
        "*://*.example2.com/*"
    ],
    "disabled_defaults": [
        ""
    ]
}"#;

    let defaults: Vec<String> = serde_json::from_str(constants::DEFAULT_BLOCKLIST).unwrap();
    let blocklist_path: String = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\user_blocklist.json";
    let mut blocklist_file = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(false)
        .open(&blocklist_path)
        .unwrap();

    if blocklist_file.metadata().unwrap().len() == 0 {
        blocklist_file.write_all(example_blocklist.as_bytes()).ok();
    }

    let blocklist_string = std::fs::read_to_string(&blocklist_path).unwrap();

    let blocklist = match serde_json::from_str::<UserBlocklist>(&blocklist_string) {
        Ok(config) => config,
        Err(_) => {
            blocklist_file.set_len(0).ok();
            blocklist_file.write_all(example_blocklist.as_bytes()).ok();
            serde_json::from_str::<UserBlocklist>(example_blocklist).unwrap()
        }
    };

    let final_url_blocklist = defaults
        .into_iter()
        .filter(|url| !blocklist.disabled_defaults.contains(url))
        .chain(blocklist.blocked);

    for url in final_url_blocklist {
        unsafe {
            let _ = webview_window.AddWebResourceRequestedFilter(
                PCWSTR(utils::create_utf_string(url).as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            );
        };
    }
}
