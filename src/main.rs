#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
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
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use regex::Regex;
use std::sync::{Arc, Mutex};

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

fn main() {
    utils::kill_glorps(); //NOOOOO

    let discord_client: Mutex<Option<DiscordIpcClient>> = Mutex::new(None);
    let config = Arc::new(Mutex::new(config::Config::load()));
    let token: *mut EventRegistrationToken = std::ptr::null_mut();

    let mut args: String = String::new();

    for flag in constants::DEFAULT_FLAGS {
        args.push_str(flag);
        args.push(' ');
    }

    if config.lock().unwrap().get("uncapFps").unwrap_or(true) {
        args.push_str("--disable-frame-rate-limit")
    }

    if config.lock().unwrap().get("discordRPC").unwrap_or(false) {
            match DiscordIpcClient::new(constants::DISCORD_CLIENT_ID) {
                Ok(mut client) => {
                    if client.connect().is_ok() {
                        *discord_client.lock().unwrap() = Some(client);
                    } else {
                        eprintln!("Failed to connect Discord IPC");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to create Discord IPC client: {}", e);
                }
            }
        
    }

    unsafe {
        let hwnd: HWND = window::create_window(config.lock().unwrap().get::<String>("startMode").unwrap_or_else(|| String::from("Borderless Fullscreen")).as_str());
        let webview2_components = window::create_webview2(hwnd, args);

        modules::priority::set(
            config
                .lock()
                .unwrap()
                .get::<String>("webviewPriority")
                .unwrap_or(String::from("Normal"))
                .as_str(),
        );

        inject::hook_webview2(config.lock().unwrap().get("hardFlip").unwrap_or(false));
        let controller = webview2_components.0;
        let env = webview2_components.1;

        let webview_window: ICoreWebView2 = controller.CoreWebView2().unwrap();

        #[cfg(not(debug_assertions))]
        {
            if config.lock().unwrap().get("checkUpdates").unwrap_or(false) {
                installer::check_update();
            }
        }

        let controller = controller.cast::<ICoreWebView2Controller4>().unwrap();
        let webview_window = webview_window.cast::<ICoreWebView2_22>().unwrap();

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
            webview2_settings.SetUserAgent(w!("Electron"))?;
            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("Failed to set WebView2 settings: {}", e);
        }

        if config.lock().unwrap().get("userscripts").unwrap_or(false) {
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

        if config.lock().unwrap().get("blocklist").unwrap_or(true) {
            blocklist = modules::blocklist::load(&webview_window)
        };
        if config.lock().unwrap().get("swapper").unwrap_or(true) {
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
                            .unwrap_or(2.0)
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
                                    PCWSTR(utils::create_utf_string(&format!("{{\"rate\":{}}}", config_clone.lock().unwrap().get::<f32>("throttle").unwrap_or(1.0))).as_ptr()),
                                    None,
                                ).ok();
                            } else {
                                 webview_window.unwrap().CallDevToolsProtocolMethod(
                                    w!("Emulation.setCPUThrottlingRate"),
                                    PCWSTR(utils::create_utf_string(&format!("{{\"rate\":{}}}", config_clone.lock().unwrap().get::<f32>("inMenuThrottle").unwrap_or(2.0))).as_ptr()),

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
                            Some(&"rpcUpdate") => {
                                if parts.len() >= 3 {
                                    let details = "Krunker";
                                    let state = format!("{} on {}", parts[1], parts[2]);
                                    if let Some(client) = &mut *discord_client.lock().unwrap() {
                                        let activity = activity::Activity::new()
                                            .details(details)
                                            .state(&state)
                                            .assets(activity::Assets::new());

                                        if let Err(e) = client.set_activity(activity) {
                                            eprintln!("Failed to set rpc activity: {}", e);
                                        }
                                    }
                                } else {
                                    eprintln!("Invalid rpcUpdate message format: {}", message_string);
                                }
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
