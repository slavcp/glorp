use regex::Regex;
use walkdir::WalkDir;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::{
    Win32::{
        Foundation::*,
        System::Com::{StructuredStorage::*, *},
    },
    core::*,
};

use crate::utils;

pub fn load(main_window: &ICoreWebView2_22) -> Vec<(Regex, IStream)> {
    let swap_dir = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\swapper";
    std::fs::create_dir_all(&swap_dir).unwrap_or_default();

    let mut swaps = Vec::new();

    for entry in WalkDir::new(&swap_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let relative_path = entry
                .path()
                .strip_prefix(&swap_dir)
                .unwrap()
                .to_str()
                .unwrap()
                .replace("\\", "/");
            unsafe {
                let url = (
                    format!("*://krunker.io/{}*", relative_path),
                    format!("*://*.krunker.io/{}*", relative_path),
                );

                for url_part in [&url.0, &url.1] {
                    if let Err(e) = main_window.AddWebResourceRequestedFilterWithRequestSourceKinds(
                        PCWSTR(utils::create_utf_string(url_part).as_ptr()),
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                        COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                    ) {
                        eprintln!("Failed to add web resource requested filter: {}", e);
                    }
                }
            }
            unsafe {
                let regex = format!(r#"^.*://(?:[^/]+\.)*krunker.io/{}.*$"#, relative_path);
                let regex = Regex::new(&regex).unwrap();
                let file_content =
                    std::fs::read(entry.path().display().to_string().replace("\\", "/")).unwrap();
                let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true).unwrap();
                stream
                    .Write(
                        file_content.as_ptr() as *const _,
                        file_content.len() as u32,
                        None,
                    )
                    .unwrap();
                stream.Seek(0, STREAM_SEEK_SET, None).unwrap();
                swaps.push((regex, stream));
            }
        }
    }
    swaps
}
