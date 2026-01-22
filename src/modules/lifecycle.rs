#![allow(dead_code)]
use crate::{
    constants,
    utils::{self, create_utf_string},
};

use std::{env, fs, io, io::*, process};
use windows::{
    Win32::Foundation::*,
    Win32::System::{DataExchange::COPYDATASTRUCT, Threading::CreateMutexW},
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::*},
    core::*,
};

pub fn read_js_bundle() -> io::Result<String> {
    let current_exe = env::current_exe().unwrap();
    let dir = current_exe.parent().unwrap();

    let frontend_path = dir.join("resources/bundle.js");
    let mut js_bundle = fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(false)
        .open(&frontend_path)?;

    if let Ok(metadata) = js_bundle.metadata()
        && metadata.len() > 0
    {
        let mut content = String::new();
        if js_bundle.read_to_string(&mut content).is_ok() {
            return Ok(content);
        }
    }

    Err(io::Error::other("file not found, resorting to included js"))
}

fn string_download(url: &str) -> std::result::Result<String, ureq::Error> {
    let response = ureq::get(url).call()?;
    let mut buf = String::new();
    response.into_body().as_reader().read_to_string(&mut buf)?;

    Ok(buf)
}

pub fn check_minor_update() -> Option<String> {
    let Ok(new_ver) = string_download(constants::JS_VERSION_URL) else {
        return None;
    };
    let resouce_folder = env::current_exe().unwrap().parent().unwrap().join("resources");

    let current_ver =
        fs::read_to_string(resouce_folder.join("bundle_version")).unwrap_or_else(|_| String::from("0.0.0"));

    let parsed_current_ver = match semver::Version::parse(current_ver.trim()) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("can't parse current version");
            return None;
        }
    };
    if semver::Version::parse(&new_ver).unwrap() > parsed_current_ver {
        let Ok(new_js) = string_download(constants::JS_BUNDLE_URL) else {
            return None;
        };
        utils::atomic_write(&resouce_folder.join("bundle.js"), &new_js).ok()?;
        utils::atomic_write(&resouce_folder.join("bundle_version"), &new_ver).ok()?;
        *crate::JS_VERSION.lock().unwrap() = new_ver;
        Some(new_js)
    } else {
        *crate::JS_VERSION.lock().unwrap() = current_ver;
        None
    }
}

pub fn check_major_update() {
    // fetch latest version form github
    let Ok(buf) = string_download(constants::UPDATE_URL) else {
        return;
    };

    let json = serde_json::from_str::<serde_json::Value>(&buf).unwrap();
    let newest_version = match json["tag_name"].as_str() {
        Some(v) => v,
        None => return,
    };
    if semver::Version::parse(newest_version).unwrap() <= semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap() {
        return;
    };

    // download
    let download_url = match json["assets"][0]["browser_download_url"].as_str() {
        Some(url) => url,
        None => return,
    };

    let mut output_path = env::current_exe().expect("can't get exe path");
    output_path.pop();
    output_path.push(format!("version.{}.msi", newest_version));

    let res = match ureq::get(download_url).call() {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Failed to download: {:?}", e);
            return;
        }
    };

    let mut file = match fs::File::create(&output_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Failed to create file: {:?}", e);
            return;
        }
    };

    if let Err(e) = io::copy(&mut res.into_body().as_reader(), &mut file) {
        eprintln!("Failed to write to file: {:?}", e);
        return;
    }
    drop(file);
    unsafe {
        if let IDYES = MessageBoxW(
            None,
            w!("A new version is available, update?"),
            w!("Update available"),
            MB_ICONQUESTION | MB_YESNO,
        ) {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(crate::utils::create_utf_string(output_path.to_string_lossy()).as_ptr()),
                w!("/q"),
                None,
                SW_NORMAL,
            );
            process::exit(0);
        }
    }
}

pub fn installer_cleanup() -> io::Result<()> {
    let current_dir = env::current_dir()?;

    for entry in fs::read_dir(&current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file()
            && let Some(extension) = path.extension()
            && extension.eq_ignore_ascii_case("msi")
        {
            fs::remove_file(&path).ok();
        }
    }
    Ok(())
}

pub fn set_panic_hook() -> io::Result<()> {
    let current_dir = env::current_dir()?;
    let log_file_path = current_dir.join("crash_log.txt");

    std::panic::set_hook(Box::new(move |panic_info| {
        let crash_message = format!(
            "Location: {}\n\
            Message: {}\n\
            \nStack Trace:\n{}\n",
            {
                let loc_string = panic_info
                    .location()
                    .map(|loc| loc.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                loc_string.to_string()
            },
            panic_info
                .payload()
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_info.payload().downcast_ref::<&str>().copied())
                .unwrap_or("<unknown>"),
            std::backtrace::Backtrace::force_capture()
        );

        fs::write(&log_file_path, &crash_message).ok();

        unsafe {
            let result = MessageBoxW(
                None,
                PCWSTR(
                    create_utf_string(format!(
                        "A crash report has been saved to:\n\
                     {}\n\n\
                     Click Yes to open the log.",
                        log_file_path.display()
                    ))
                    .as_ptr(),
                ),
                PCWSTR(create_utf_string("Application Error").as_ptr()),
                MB_YESNO | MB_ICONERROR,
            );

            if result == IDYES {
                ShellExecuteW(
                    None,
                    PCWSTR(create_utf_string("open").as_ptr()),
                    PCWSTR(create_utf_string(log_file_path.to_string_lossy()).as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOW,
                );
            }
        }
    }));
    Ok(())
}

pub fn register_instance() {
    unsafe {
        CreateMutexW(
            None,
            false,
            PCWSTR(create_utf_string(constants::INSTANCE_MUTEX).as_ptr()),
        )
        .ok();

        if GetLastError() == ERROR_ALREADY_EXISTS {
            eprintln!("Instance already running");
            let data = env::args().skip(1).collect::<Vec<String>>().join(" ");

            if data.is_empty() && FindWindowW(w!("krunker_webview_subwindow"), PCWSTR::null()).is_err() {
                process::exit(0);
            }
            let data_bytes = data.as_bytes();
            let copy_data = COPYDATASTRUCT {
                dwData: 0,
                cbData: data_bytes.len() as u32,
                lpData: data_bytes.as_ptr() as *mut std::ffi::c_void,
            };
            if let Ok(hwnd) = FindWindowExW(None, None, w!("krunker_webview"), PCWSTR::null()) {
                SendMessageW(
                    hwnd,
                    WM_COPYDATA,
                    Some(WPARAM(0)),
                    Some(LPARAM(&copy_data as *const COPYDATASTRUCT as isize)),
                );
            } else {
                SendMessageW(
                    FindWindowW(w!("krunker_webview_subwindow"), PCWSTR::null()).unwrap(),
                    WM_COPYDATA,
                    None,
                    Some(LPARAM(&copy_data as *const COPYDATASTRUCT as isize)),
                );
            }
            process::exit(0);
        }
    }
}
