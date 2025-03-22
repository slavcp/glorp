use std::io::Read;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;

pub fn load(webview: &ICoreWebView2) -> windows::core::Result<()> {
    let scripts_dir = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\scripts";

    if let Ok(entries) = std::fs::read_dir(scripts_dir) {
        for entry in entries.flatten() {
            if !match entry.path().extension() {
                Some(ext) => ext.eq_ignore_ascii_case("js"),
                None => false,
            } {
                continue;
            }

            let mut file = std::fs::File::open(entry.path())?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            unsafe {
                webview.AddScriptToExecuteOnDocumentCreated(
                    PCWSTR(super::utils::create_utf_string(&content).as_ptr()),
                    None,
                )?
            }
        }
    }

    Ok(())
}
