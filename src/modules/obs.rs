use crate::{debug_print, utils};
use std::{fs, path::PathBuf};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use windows::{
    Win32::{UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL},
    core::*,
};

pub fn obs_plugin_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for variable in ["PROGRAMFILES", "PROGRAMFILES(X86)"] {
        if let Ok(root) = std::env::var(variable) {
            let directory = PathBuf::from(root).join("obs-studio").join("obs-plugins").join("64bit");
            if directory.is_dir() {
                paths.push(directory.join("obs-glorp-capture.dll"));
            }
        }
    }
    paths
}

pub fn set_plugin_installed(webview: &ICoreWebView2, install: bool) {
    let (ok, message) = match (|| -> std::io::Result<String> {
        let paths = obs_plugin_paths();
        let dest = paths
            .first()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "OBS was not found in Program Files"))?;

        if install {
            let source = std::env::current_exe()?.parent().unwrap().join("resources").join("obs-glorp-capture.dll");

            if !source.exists() {
                return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "bundled OBS plugin is missing"));
            }
            fs::copy(source, dest)?;
            Ok(format!("OBS plugin installed to {}", dest.display()))
        } else if let Some(path) = paths.iter().find(|p| p.exists()) {
            fs::remove_file(path)?;
            Ok("OBS plugin removed".to_string())
        } else {
            Ok("OBS plugin was not installed".to_string())
        }
    })() {
        Ok(msg) => (true, msg),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let operation = if install { "--install-obs-plugin" } else { "--uninstall-obs-plugin" };
            let launched = std::env::current_exe().is_ok_and(|exe| unsafe {
                let exe = utils::create_utf_string(exe.to_string_lossy());
                let args = utils::create_utf_string(operation);
                ShellExecuteW(None, w!("runas"), PCWSTR(exe.as_ptr()), PCWSTR(args.as_ptr()), None, SW_SHOWNORMAL).0 as usize > 32
            });

            if launched {
                (true, "Windows administrator permission requested for the OBS plugin operation".to_string())
            } else {
                (false, format!("OBS plugin operation failed: {e}"))
            }
        }
        Err(e) => (false, format!("OBS plugin operation failed: {e}")),
    };

    if !ok || message.starts_with("Windows administrator") {
        crate::CONFIG.lock().unwrap().set("obsCapturePlugin", false);
    }

    debug_print!("{}", &message);

    let payload = serde_json::json!({ "type": "obs-plugin", "ok": ok, "message": message });
    if let Ok(json) = serde_json::to_string(&payload) {
        unsafe {
            webview.PostWebMessageAsJson(PCWSTR(utils::create_utf_string(json).as_ptr())).ok();
        }
    }
}

pub fn run_plugin_operation(install: bool) -> std::io::Result<()> {
    let paths = obs_plugin_paths();
    let dest = paths
        .first()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "OBS was not found in Program Files"))?;

    if install {
        let source = std::env::current_exe()?.parent().unwrap().join("resources").join("obs-glorp-capture.dll");
        fs::copy(source, dest).map(drop)
    } else {
        paths.into_iter().filter(|p| p.exists()).try_for_each(fs::remove_file)
    }
}

pub fn handle_cli_flags() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let install = args.iter().any(|arg| arg == "--install-obs-plugin");
    let uninstall = args.iter().any(|arg| arg == "--uninstall-obs-plugin");

    if install || uninstall {
        if let Err(_error) = run_plugin_operation(install) {
            debug_print!("elevated OBS plugin operation failed: {_error:?}");
        }
        return true;
    }

    false
}
