use std::{collections::HashMap, env, fs, path::PathBuf};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::{
    Win32::{
        Foundation::*,
        System::Com::{StructuredStorage::*, *},
    },
    core::*,
};

use crate::utils;

fn recurse_swap(root_dir: PathBuf, swap_dir: PathBuf, window: &ICoreWebView2) -> Option<HashMap<String, IStream>> {
    let mut swaps = HashMap::new();

    for entry in fs::read_dir(&swap_dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        let file_type = entry.file_type().ok()?;
        if file_type.is_dir() {
            if let Some(sub_swaps) = recurse_swap(root_dir.clone(), path, window) {
                swaps.extend(sub_swaps);
            }
        } else if file_type.is_file() {
            let relative_path = entry
                .path()
                .strip_prefix(&root_dir)
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
                    if let Err(e) = window.AddWebResourceRequestedFilter(
                        PCWSTR(utils::create_utf_string(url_part).as_ptr()),
                        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    ) {
                        eprintln!("Failed to add web resource requested filter: {}", e);
                    }
                }
            }
            unsafe {
                let file_content = fs::read(entry.path()).unwrap();
                let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true).unwrap();
                stream
                    .Write(file_content.as_ptr() as *const _, file_content.len() as u32, None)
                    .unwrap();
                stream.Seek(0, STREAM_SEEK_SET, None).ok();
                swaps.insert(relative_path, stream);
            }
        }
    }
    Some(swaps)
}

pub fn load(window: &ICoreWebView2) -> HashMap<String, IStream> {
    let swap_dir = PathBuf::from(env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\swapper");
    fs::create_dir_all(&swap_dir).unwrap_or_default();
    let swaps = recurse_swap(swap_dir.clone(), swap_dir, window);
    swaps.unwrap()
}
