#![allow(dead_code)]
use std::io::Read;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::{Win32::UI::WindowsAndMessaging::*, core::*};

const UPDATE_URL: &str = "https://api.github.com/repos/slavcp/glorp/releases/latest";

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

        let mut file = match std::fs::File::create(&output_path) {
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

        if let Err(e) = std::io::copy(&mut res.into_body().as_reader(), &mut file) {
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
                std::process::exit(0);
            }
        }
    });
}
