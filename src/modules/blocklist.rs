use std::{collections::HashSet, env, fs, io::Write};

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;

use crate::constants;
use crate::utils;

#[derive(serde::Deserialize, serde::Serialize)]
struct UserBlocklist {
    blocked: HashSet<String>,
    disabled_defaults: HashSet<String>,
}

fn load_defaults(webview_window: &ICoreWebView2, defaults: Vec<String>) {
    for url in defaults {
        unsafe {
            let _ = webview_window.AddWebResourceRequestedFilter(
                PCWSTR(utils::create_utf_string(url).as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            );
        };
    }
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
    let blocklist_path: String = env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\user_blocklist.json";
    let mut blocklist_file = if let Ok(file) = fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(false)
        .open(&blocklist_path)
    {
        file
    } else {
        eprintln!("can't open blocklist file");
        load_defaults(webview_window, defaults);
        return;
    };

    if blocklist_file.metadata().unwrap().len() == 0 {
        blocklist_file.write_all(example_blocklist.as_bytes()).ok();
    }

    let blocklist_string = if let Ok(blocklist_string) = fs::read_to_string(&blocklist_path) {
        blocklist_string
    } else {
        eprintln!("can't read user blocklist file");
        blocklist_file.set_len(0).ok();
        blocklist_file.write_all(example_blocklist.as_bytes()).ok();
        load_defaults(webview_window, defaults);
        return;
    };

    let blocklist = match serde_json::from_str::<UserBlocklist>(&blocklist_string) {
        Ok(config) => config,
        Err(_) => {
            eprintln!("can't parse user blocklist file");
            blocklist_file.set_len(0).ok();
            blocklist_file.write_all(example_blocklist.as_bytes()).ok();
            load_defaults(webview_window, defaults);
            return;
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
