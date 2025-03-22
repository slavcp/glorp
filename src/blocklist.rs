use regex::Regex;
use std::fs::*;
use std::io::*;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;

pub fn load(webview_window: &ICoreWebView2_22) -> Vec<Regex> {
    let blocklist_path: String =
        std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\blocklist.json";
    let mut blocklist_file = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(false)
        .open(&blocklist_path)
        .unwrap();

    if blocklist_file.metadata().unwrap().len() == 0 {
        blocklist_file
            .write_all(super::constants::DEFAULT_BLOCKLIST.as_bytes())
            .unwrap();
    }

    let blocklist_string = std::fs::read_to_string(&blocklist_path).unwrap();

    let blocklist = match serde_json::from_str::<Vec<String>>(&blocklist_string) {
        Ok(blocklist) => blocklist,
        Err(_) => serde_json::from_str::<Vec<String>>(super::constants::DEFAULT_BLOCKLIST).unwrap(),
    };

    let mut blocklist_regex: Vec<Regex> = Vec::<Regex>::new();

    for url in &blocklist {
        unsafe {
            if let Err(e) = webview_window.AddWebResourceRequestedFilterWithRequestSourceKinds(
                PCWSTR(super::utils::create_utf_string(url).as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
            ) {
                eprintln!("Failed to add web resource requested filter: {}", e);
            }
        }
        // donald trump please save me
        let pattern = url.replace("*", ".*");
        let pattern = format!("^{}$", pattern);
        let regex = Regex::new(&pattern).unwrap();
        blocklist_regex.push(regex);
    }
    blocklist_regex
}
