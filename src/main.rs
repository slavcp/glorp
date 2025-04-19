#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use regex::Regex;
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::{
    Win32::{
        Foundation::*,
        System::{Com::*, WinRT::*},
        UI::WindowsAndMessaging::*,
    },
    core::*,
};
mod config;
mod constants;
mod inject;
mod installer;
mod utils;
mod window;

mod modules {
    pub mod blocklist;
    pub mod priority;
    pub mod swapper;
    pub mod userscripts;
}

// > memory safe langauge
// > unsafe

use std::sync::{Arc, Mutex};

fn main() {
    utils::kill_glorps(); //NOOOOO

    let config = Arc::new(Mutex::new(config::Config::load()));
    let token: *mut EventRegistrationToken = std::ptr::null_mut();

    let mut args: String = String::new();

    for flag in constants::DEFAULT_FLAGS {
        args.push_str(flag);
        args.push(' ');
    }

    if config.lock().unwrap().get("uncapFps").unwrap() {
        args.push_str("--disable-frame-rate-limit")
    }

    unsafe {
        let hwnd: HWND = window::create_window();
        let webview2_components = window::create_webview2(hwnd, args);

        modules::priority::set(
            config
                .lock()
                .unwrap()
                .get::<String>("webviewPriority")
                .unwrap()
                .as_str(),
        );

        inject::hook_webview2(&config);
        let controller = webview2_components.0;
        let env = webview2_components.1;

        let webview_window: ICoreWebView2 = controller.CoreWebView2().unwrap();

        #[cfg(not(debug_assertions))]
        {
            if config.lock().unwrap().get("checkUpdates").unwrap() {
                installer::check_update();
            }
        }

        let mut rect: RECT = RECT::default();
        let controller = controller.cast::<ICoreWebView2Controller4>().unwrap();
        let webview_window = webview_window.cast::<ICoreWebView2_22>().unwrap();

        GetWindowRect(hwnd, &mut rect).ok();
        controller.SetBounds(rect).ok();

        controller.SetAllowExternalDrop(false).unwrap();
        controller
            .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                A: 255,
                R: 0,
                G: 0,
                B: 0,
            })
            .ok();

        let result = (|| -> std::result::Result<(), windows::core::Error> {
            let webview2_settings = webview_window
                .Settings()
                .unwrap()
                .cast::<ICoreWebView2Settings9>()
                .unwrap();

            webview2_settings.SetIsReputationCheckingRequired(false)?;
            webview2_settings.SetIsSwipeNavigationEnabled(false)?;
            webview2_settings.SetIsPinchZoomEnabled(false)?;
            webview2_settings.SetIsPasswordAutosaveEnabled(false)?;
            webview2_settings.SetIsGeneralAutofillEnabled(false)?;
            webview2_settings.SetAreBrowserAcceleratorKeysEnabled(false)?;
            webview2_settings.SetAreDefaultContextMenusEnabled(false)?;
            webview2_settings.SetIsZoomControlEnabled(false)?;
            webview2_settings.SetIsBuiltInErrorPageEnabled(false)?;
            webview2_settings.SetUserAgent(w!("Electron"))?;
            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("Failed to set WebView2 settings: {}", e);
        }

        if config.lock().unwrap().get("userscripts").unwrap() {
            if let Err(e) = modules::userscripts::load(&webview_window) {
                eprintln!("Failed to load userscripts: {}", e);
            }
        }

        #[rustfmt::skip]
        webview_window
            .AddScriptToExecuteOnDocumentCreated(
                PCWSTR(utils::create_utf_string(include_str!("../target/bundle.js")).as_ptr()),
                None,
            )
            .ok();

        webview_window.Navigate(w!("https://krunker.io")).ok();

        // auto accept permission requests
        webview_window
            .add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(
                    move |_, args: Option<ICoreWebView2PermissionRequestedEventArgs>| {
                        args.unwrap()
                            .SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)
                            .ok();
                        Ok(())
                    },
                )),
                token,
            )
            .ok();

        let mut blocklist: Vec<Regex> = Vec::new();
        let mut swaps: Vec<(Regex, IStream)> = Vec::new();

        if config.lock().unwrap().get("blocklist").unwrap_or_default() {
            blocklist = modules::blocklist::load(&webview_window)
        };
        if config.lock().unwrap().get("swapper").unwrap_or_default() {
            swaps = modules::swapper::load(&webview_window)
        };

        webview_window
            .AddWebResourceRequestedFilterWithRequestSourceKinds(
                w!("*://matchmaker.krunker.io/*"),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
            )
            .unwrap();

        webview_window.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(
                move |webview: Option<ICoreWebView2>,
                      args: Option<ICoreWebView2WebResourceRequestedEventArgs>| {
                    if let Some(args) = args {
                        let request: ICoreWebView2WebResourceRequest = args.Request()?;
                        let mut uri_string = utils::create_utf_string("");
                        let uri = uri_string.as_mut_ptr() as *mut PWSTR;
                        request.Uri(uri)?;
                        let uri = uri.as_ref().unwrap().to_string().unwrap();
                        for regex in &blocklist {
                            if regex.is_match(&uri) {
                                request.SetUri(PCWSTR::null())?;
                                return Ok(());
                            }
                        }

                        for (pattern, stream) in &swaps {
                            if pattern.is_match(&uri) {
                                let response = env.CreateWebResourceResponse(
                                    stream,
                                    200,
                                    w!("OK"),
                                    w!("Access-Control-Allow-Origin: *"),
                                )?;
                                args.SetResponse(Some(&response))?;

                                return Ok(());
                            }
                        }
                        if uri.contains("matchmaker.krunker.io/game-info?game") {
                            webview
                                .unwrap()
                                .PostWebMessageAsJson(w!("\"game-updated\""))
                                .unwrap();
                        }
                    }

                    Ok(())
                },
            )),
            token,
        ).unwrap();

        let widget_wnd = Some(utils::find_child_window_by_class(
            FindWindowW(w!("krunker_webview"), PCWSTR::null()).unwrap(),
            "Chrome_RenderWidgetHostHWND",
        ));
        let config_clone = Arc::clone(&config);

        webview_window
            .CallDevToolsProtocolMethod(
                w!("Emulation.setCPUThrottlingRate"),
                PCWSTR(
                    utils::create_utf_string(&format!(
                        "{{\"rate\":{}}}",
                        config_clone
                            .lock()
                            .unwrap()
                            .get::<f32>("inMenuThrottle")
                            .unwrap()
                    ))
                    .as_ptr(),
                ),
                None,
            )
            .ok();

        webview_window.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(
                move |webview_window, args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
                    if let Some(args) = args {
                        let mut message_vec = utils::create_utf_string("");
                        let message = message_vec.as_mut_ptr() as *mut PWSTR;
                        if let Err(e) = args.TryGetWebMessageAsString(message) {
                            eprintln!("Failed to get web message as string: {}", e);
                        }

                        let message_string = message.as_ref().unwrap().to_string().unwrap();
                        let message = message_string.as_str(); // fire

                        let parts: Vec<&str> = message.split(',').map(|s| s.trim()).collect();
                        match parts.first() {
                            Some(&"setConfig") => {
                                if parts.len() >= 3 {
                                    let setting = parts[1];
                                    let value = if let Ok(bool_val) = parts[2].parse::<bool>() {
                                        serde_json::Value::Bool(bool_val)
                                    } else if let Ok(float_val) = parts[2].parse::<f64>() {
                                        serde_json::Value::Number(serde_json::Number::from_f64((float_val * 100.0).round() / 100.0).unwrap())
                                    } else {
                                        serde_json::Value::String(parts[2].to_string())
                                    };
                                    config_clone.lock().unwrap().set(setting, value);
                                }
                            }
                            Some(&"getConfig") => {
                                config_clone.lock().unwrap().send_config(&webview_window.unwrap());
                            }
                            Some(&"pointerLockChange") => {
                                let value = parts[1].parse::<bool>().unwrap_or(false);

                                PostMessageW(
                                    widget_wnd,
                                    WM_USER,
                                    WPARAM(value as usize),
                                    LPARAM(0),
                                ).ok();

                                if value {
                                 webview_window.unwrap().CallDevToolsProtocolMethod(
                                    w!("Emulation.setCPUThrottlingRate"),
                                    PCWSTR(utils::create_utf_string(&format!("{{\"rate\":{}}}", config_clone.lock().unwrap().get::<f32>("throttle").unwrap())).as_ptr()),
                                    None,
                                ).ok();
                            } else {
                                 webview_window.unwrap().CallDevToolsProtocolMethod(
                                    w!("Emulation.setCPUThrottlingRate"),
                                    PCWSTR(utils::create_utf_string(&format!("{{\"rate\":{}}}", config_clone.lock().unwrap().get::<f32>("inMenuThrottle").unwrap())).as_ptr()),

                                    None,
                                ).ok();
                            }

                            }
                            Some(&"close") => {
                                PostQuitMessage(0);
                            }
                            Some(&"open") => {
                                std::process::Command::new("cmd")
                                    .args(["/C", "start", "", parts[1]])
                                    .spawn()
                                    .ok();
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                },
            )),
            token,
        ).ok();

        controller
            .add_AcceleratorKeyPressed(
                &AcceleratorKeyPressedEventHandler::create(Box::new(
                    move |_, args: Option<ICoreWebView2AcceleratorKeyPressedEventArgs>| {
                        let mut pressed_key: u32 = 0;
                        let mut key_event_kind: COREWEBVIEW2_KEY_EVENT_KIND =
                            COREWEBVIEW2_KEY_EVENT_KIND::default();
                        let args: ICoreWebView2AcceleratorKeyPressedEventArgs = args.unwrap();

                        args.KeyEventKind(&mut key_event_kind)?;
                        args.VirtualKey(&mut pressed_key)?;
                        if key_event_kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
                            return Ok(());
                        }
                        match VIRTUAL_KEY(pressed_key as u16) {
                            VK_F4 | VK_F6 => {
                                webview_window.Navigate(w!("https://krunker.io")).ok();
                                PostMessageW(
                                    widget_wnd,
                                    WM_USER,
                                    WPARAM(false as usize),
                                    LPARAM(0),
                                )
                                .ok();
                            }
                            VK_F5 => {
                                webview_window.Reload().ok();
                                PostMessageW(
                                    widget_wnd,
                                    WM_USER,
                                    WPARAM(false as usize),
                                    LPARAM(0),
                                )
                                .ok();
                            }
                            VK_F11 => {
                                window::toggle_fullscreen(hwnd);
                            }
                            VK_F12 => {
                                webview_window.OpenDevToolsWindow().ok();
                            }

                            _ => {}
                        }
                        Ok(())
                    },
                )),
                token,
            )
            .unwrap();

        let mut msg: MSG = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    };
    // code here runs after window is closed

    config.lock().unwrap().save();
}
