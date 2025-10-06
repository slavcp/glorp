#![allow(dead_code)]
use crate::utils::create_utf_string;
use std::{env, fs, io, io::*, process};
use windows::{
    Win32::Foundation::*,
    Win32::System::Threading::CreateMutexW,
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::*},
    core::*,
};

const UPDATE_URL: &str = "https://api.github.com/repos/slavcp/glorp/releases/latest";
const INSTANCE_MUTEX_NAME: &str = "Global\\7e0f405e-fe65-493a-acf0-9719b85697cd";

pub fn check_update() {
    std::thread::spawn(|| {
        let mut response = match ureq::get(UPDATE_URL).call() {
            Ok(response) => response.into_body(),
            Err(_) => return,
        };
        let mut buf = String::new();
        response.as_reader().read_to_string(&mut buf).unwrap();

        let json = serde_json::from_str::<serde_json::Value>(&buf).unwrap();
        let newest_version = match json["tag_name"].as_str() {
            Some(v) => v,
            None => return,
        };
        if semver::Version::parse(newest_version).unwrap()
            <= semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
        {
            return;
        };

        let download_url = match json["assets"][0]["browser_download_url"].as_str() {
            Some(url) => url,
            None => return,
        };

        let mut output_path = std::env::current_exe().unwrap();
        output_path.pop();
        output_path.push(format!("version.{}.msi", newest_version));

        let res = match ureq::get(download_url).call() {
            Ok(res) => res,
            Err(e) => {
                unsafe {
                    MessageBoxW(
                        None,
                        PCWSTR(
                            crate::utils::create_utf_string(
                                format!("Failed to download: {:?}", e).as_str(),
                            )
                            .as_ptr(),
                        ),
                        w!("Download Error"),
                        MB_ICONERROR | MB_SYSTEMMODAL,
                    );
                }
                return;
            }
        };

        let mut file = match fs::File::create(&output_path) {
            Ok(file) => file,
            Err(e) => {
                unsafe {
                    MessageBoxW(
                        None,
                        PCWSTR(
                            crate::utils::create_utf_string(
                                format!("Failed to create file: {:?}", e).as_str(),
                            )
                            .as_ptr(),
                        ),
                        w!("Download Error"),
                        MB_ICONERROR | MB_SYSTEMMODAL,
                    );
                }
                return;
            }
        };

        if let Err(e) = io::copy(&mut res.into_body().as_reader(), &mut file) {
            panic!("Failed to download: {:?}", e)
        }
        drop(file);
        unsafe {
            if let MESSAGEBOX_RESULT(6) = MessageBoxW(
                None,
                w!("A new version is available, update?"),
                w!("Update available"),
                MB_ICONQUESTION | MB_YESNO,
            ) {
                ShellExecuteW(
                    None,
                    w!("open"),
                    PCWSTR(crate::utils::create_utf_string(output_path.to_str().unwrap()).as_ptr()),
                    w!("/q"),
                    None,
                    SW_NORMAL,
                );
                process::exit(0);
            }
        }
    });
}

pub fn installer_cleanup() -> io::Result<()> {
    let current_dir = env::current_dir()?;

    for entry in fs::read_dir(&current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension.eq_ignore_ascii_case("msi") {
                    fs::remove_file(&path).ok();
                }
            }
        }
    }
    Ok(())
}

pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let exe_path =
            std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("unknown_path"));
        let log_dir = exe_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("./"));
        let log_file_path = log_dir.join("crash_log.txt");

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

        std::fs::write(&log_file_path, &crash_message).ok();

        unsafe {
            let result = MessageBoxW(
                None,
                PCWSTR(
                    create_utf_string(&format!(
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
                    PCWSTR(create_utf_string(&log_file_path.to_string_lossy()).as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOW,
                );
            }
        }
    }));
}

pub fn register_instance() {
    unsafe {
        let mutex = CreateMutexW(
            None,
            false,
            PCWSTR(create_utf_string(INSTANCE_MUTEX_NAME).as_ptr()),
        );
        if mutex.is_ok() && GetLastError() == ERROR_ALREADY_EXISTS {
            eprintln!("Instance already running");
            std::process::exit(0);
        }
    }
}
