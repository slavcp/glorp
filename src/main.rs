#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use regex::Regex;
use std::sync::{Arc, Mutex};
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
        let (mut main_window, env) = window::Window::new(
            config
                .lock()
                .unwrap()
                .get::<String>("startMode")
                .unwrap_or_else(|| String::from("Borderless Fullscreen"))
                .as_str(),
            true,
            args
        );

        modules::priority::set(
            config
                .lock()
                .unwrap()
                .get::<String>("webviewPriority")
                .unwrap_or(String::from("Normal"))
                .as_str(),
        );

        let mut webview_pid: u32 = 0;
        main_window
            .webview
            .BrowserProcessId(&mut webview_pid)
            .unwrap();

        inject::hook_webview2(
            config.lock().unwrap().get("hardFlip").unwrap_or(false),
            webview_pid,
        );

        #[cfg(not(debug_assertions))]
        {
            if config.lock().unwrap().get("checkUpdates").unwrap_or(false) {
                installer::check_update();
            }
        }

        if config.lock().unwrap().get("userscripts").unwrap_or(false) {
            if let Err(e) = modules::userscripts::load(&main_window.webview) {
                eprintln!("Failed to load userscripts: {}", e);
            }
        }

        #[rustfmt::skip]
        main_window.webview
            .AddScriptToExecuteOnDocumentCreated(
                PCWSTR(utils::create_utf_string(include_str!("../target/bundle.js")).as_ptr()),
                None,
            )
            .ok();

        main_window.webview.Navigate(w!("https://krunker.io")).ok();

        // auto accept permission requests
        main_window
            .webview
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
            blocklist = modules::blocklist::load(&main_window.webview)
        };
        if config.lock().unwrap().get("swapper").unwrap_or(true) {
            swaps = modules::swapper::load(&main_window.webview)
        };

        {
            let url = "*://matchmaker.krunker.io/game-info*";
            main_window
                .webview
                .AddWebResourceRequestedFilterWithRequestSourceKinds(
                    PCWSTR(utils::create_utf_string(url).as_ptr()),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                )
                .unwrap();
        }

        main_window.webview.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(
                move |webview: Option<ICoreWebView2>,
                      args: Option<ICoreWebView2WebResourceRequestedEventArgs>| {
                    if let Some(args) = args {
                        let request: ICoreWebView2WebResourceRequest = args.Request()?;
                        let mut uri_string = utils::create_utf_string("");
                        let uri = uri_string.as_mut_ptr() as *mut PWSTR;
                        request.Uri(uri)?;
                        let uri_string = uri.as_ref().unwrap().to_string().unwrap();
                        for regex in &blocklist {
                            if regex.is_match(&uri_string) {
                                request.SetUri(PCWSTR::null())?;
                                return Ok(());
                            }
                        }

                        for (pattern, stream) in &swaps {
                            if pattern.is_match(&uri_string) {
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
                        if uri_string.contains("matchmaker.krunker.io/game-info?game") || uri_string.contains("lobby-ranked") {
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

        if config.lock().unwrap().get("rampBoost").unwrap_or(false) {
            PostMessageW(widget_wnd, WM_APP, WPARAM(1), LPARAM(0)).ok();
        }

        let config_clone = Arc::clone(&config);

        fn set_cpu_throttling_inmenu(webview: &ICoreWebView2, cfg: &Arc<Mutex<config::Config>>) {
            unsafe {
                webview
                    .CallDevToolsProtocolMethod(
                        w!("Emulation.setCPUThrottlingRate"),
                        PCWSTR(
                            utils::create_utf_string(&format!(
                                "{{\"rate\":{}}}",
                                cfg.lock()
                                    .unwrap()
                                    .get::<f32>("inMenuThrottle")
                                    .unwrap_or(2.0)
                            ))
                            .as_ptr(),
                        ),
                        None,
                    )
                    .ok();
            }
        }

        unsafe fn set_cpu_throttling_ingame(
            webview: &ICoreWebView2,
            cfg: &Arc<Mutex<config::Config>>,
        ) {
            unsafe {
                webview
                    .CallDevToolsProtocolMethod(
                        w!("Emulation.setCPUThrottlingRate"),
                        PCWSTR(
                            utils::create_utf_string(&format!(
                                "{{\"rate\":{}}}",
                                cfg.lock().unwrap().get::<f32>("throttle").unwrap_or(1.0)
                            ))
                            .as_ptr(),
                        ),
                        None,
                    )
                    .ok();
            }
        }

        set_cpu_throttling_inmenu(&main_window.webview, &config_clone);

        main_window.webview.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(
                move |webview, _args| {
                    let version = env!("CARGO_PKG_VERSION");
                    let script = format!("window.glorpClient = window.glorpClient || {{}}; window.glorpClient.version = '{}';", version);
                    webview.unwrap().ExecuteScript(PCWSTR(utils::create_utf_string(&script).as_ptr()), None).unwrap();
                    Ok(())
                }
            )),
            token,
        ).unwrap();

        main_window
            .webview
            .add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(
                    move |webview, args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
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
                                    let setting = parts[1];
                                    let value = if let Ok(bool_val) = parts[2].parse::<bool>() {
                                        serde_json::Value::Bool(bool_val)
                                    } else if let Ok(float_val) = parts[2].parse::<f64>() {
                                        serde_json::Value::Number(
                                            serde_json::Number::from_f64(
                                                (float_val * 100.0).round() / 100.0,
                                            )
                                            .unwrap(),
                                        )
                                    } else {
                                        serde_json::Value::String(parts[2].to_string())
                                    };
                                    config_clone.lock().unwrap().set(setting, value);
                                }
                                Some(&"getConfig") => {
                                    config_clone.lock().unwrap().send_config(&webview.unwrap());
                                }
                                Some(&"pointerLock") => {
                                    let value = parts[1].parse::<bool>().unwrap_or(false);
                                    PostMessageW(
                                        widget_wnd,
                                        WM_USER,
                                        WPARAM(value as usize),
                                        LPARAM(0),
                                    )
                                    .ok();

                                    if value {
                                        set_cpu_throttling_ingame(&webview.unwrap(), &config_clone);
                                    } else {
                                        set_cpu_throttling_inmenu(&webview.unwrap(), &config_clone);
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
                                }
                                _ => {}
                            }
                        }
                        Ok(())
                    },
                )),
                token,
            )
            .ok();

        main_window
            .controller
            .clone()
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
                                main_window.webview.Navigate(w!("https://krunker.io")).ok();
                                PostMessageW(
                                    widget_wnd,
                                    WM_USER,
                                    WPARAM(false as usize),
                                    LPARAM(0),
                                )
                                .ok();
                            }
                            VK_F5 => {
                                main_window.webview.Reload().ok();
                                PostMessageW(
                                    widget_wnd,
                                    WM_USER,
                                    WPARAM(false as usize),
                                    LPARAM(0),
                                )
                                .ok();
                            }
                            VK_F11 => {
                                main_window.toggle_fullscreen();
                            }
                            VK_F12 => {
                                main_window.webview.OpenDevToolsWindow().ok();
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
