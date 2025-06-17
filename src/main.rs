#![cfg_attr(feature = "packaged", windows_subsystem = "windows")]
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
    pub mod flaglist;
    pub mod priority;
    pub mod swapper;
    pub mod userscripts;
}

// > memory safe langauges
// > unsafe

static LAST_CONNECTED_LOBBY: once_cell::sync::Lazy<Arc<Mutex<std::net::IpAddr>>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new(Mutex::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(
            127, 0, 0, 1,
        ))))
    });

fn main() {
    #[cfg(feature = "packaged")]
    {
        utils::set_panic_hook();
    }

    utils::kill("glorp.exe"); //NOOOOO

    let client_dir: String = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp";
    let swap_dir = String::from(&client_dir) + "\\swapper";
    let scripts_dir = String::from(&client_dir) + "\\scripts";
    let flaglist_path = String::from(&client_dir) + "\\flags.json";
    let blocklist_path = String::from(&client_dir) + "\\blocklist.json";
    std::fs::create_dir_all(&swap_dir).ok();
    std::fs::create_dir(&scripts_dir).ok();

    if !std::path::Path::new(&blocklist_path).exists() {
        std::fs::write(&blocklist_path, constants::DEFAULT_BLOCKLIST).ok();
    }
    if !std::path::Path::new(&flaglist_path).exists() {
        std::fs::write(&flaglist_path, constants::DEFAULT_FLAGS).ok();
    }

    let discord_client: Mutex<Option<DiscordIpcClient>> = Mutex::new(None);
    let config = Arc::new(Mutex::new(config::Config::load()));
    let token: *mut EventRegistrationToken = std::ptr::null_mut();

    let mut args = modules::flaglist::load();

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
            args,
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

        println!("Webview PID: {}", webview_pid);
        inject::hook_webview2(
            config.lock().unwrap().get("hardFlip").unwrap_or(false),
            webview_pid,
        );

        #[cfg(feature = "packaged")]
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

        #[cfg(feature = "editor-ignore")]
        {
            main_window
                .webview
                .AddScriptToExecuteOnDocumentCreated(
                    PCWSTR(utils::create_utf_string(include_str!("../target/bundle.js")).as_ptr()),
                    None,
                )
                .ok();
        }

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

        if config.lock().unwrap().get("realPing").unwrap_or(false) {
            main_window
                .webview
                .CallDevToolsProtocolMethod(w!("Network.enable"), w!("{}"), None)
                .unwrap();
            let ws_receiver = main_window
                .webview
                .GetDevToolsProtocolEventReceiver(w!("Network.webSocketCreated"))
                .unwrap();

            let handler =
                DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut params_vec = utils::create_utf_string("");
                        let params = params_vec.as_mut_ptr() as *mut PWSTR;
                        args.ParameterObjectAsJson(params)?;
                        let json = serde_json::from_str::<serde_json::Value>(
                            &params.as_ref().unwrap().to_string().unwrap(),
                        )
                        .unwrap();
                        let url = json.get("url").unwrap().to_string();
                        if url.contains("lobby-") {
                            let host = url
                                .split("://")
                                .last()
                                .unwrap()
                                .split("/")
                                .next()
                                .unwrap()
                                .to_string();
                            let resolved_ips = dns_lookup::lookup_host(&host).unwrap_or_default();
                            if let Some(ip) = resolved_ips.into_iter().next() {
                                *LAST_CONNECTED_LOBBY.lock().unwrap() = ip;
                            }
                        }
                    }
                    Ok(())
                }));

            ws_receiver
                .add_DevToolsProtocolEventReceived(&handler, token)
                .unwrap();
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

                            let parts: Vec<&str> = message_string.split(',').map(|s| s.trim()).collect();
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
                                Some(&"getInfo") => {
                                    let version = env!("CARGO_PKG_VERSION");
                                    let config = config_clone.lock().unwrap();
                                    let mut config_map = serde_json::Map::new();
                                    let args_str =
                                        std::env::args().skip(1).collect::<Vec<String>>().join(" ");
                                    if !args_str.is_empty() {
                                        config_map.insert(
                                            "launchArgs".to_string(),
                                            serde_json::Value::String(args_str),
                                        );
                                    }
                                    config_map.insert(
                                        "settings".to_string(),
                                        serde_json::json!(&*config),
                                    );
                                    config_map.insert(
                                        "version".to_string(),
                                        serde_json::Value::String(version.to_string()),
                                    );

                                    let config_json =
                                        serde_json::to_string_pretty(&config_map).unwrap();
                                    webview
                                        .unwrap()
                                        .PostWebMessageAsJson(PCWSTR(
                                            utils::create_utf_string(&config_json).as_ptr(),
                                        ))
                                        .ok();
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
                                Some(&"ping") => {
                                    println!("ping");
                                    let ip_addr = LAST_CONNECTED_LOBBY.lock().unwrap();
                                    let result = ping_rs::send_ping(
                                        &*ip_addr,
                                        std::time::Duration::from_secs(1),
                                        &[],
                                        Some(&ping_rs::PingOptions {
                                            ttl: 128,
                                            dont_fragment: true,
                                        }),
                                    );
                                    match result {
                                        Ok(reply) => {
                                            webview
                                                .unwrap()
                                                .PostWebMessageAsJson(PCWSTR(
                                                    utils::create_utf_string(&format!(
                                                        "{{\"ping\":{}}}",
                                                        reply.rtt
                                                    ))
                                                    .as_ptr(),
                                                ))
                                                .ok();
                                        }
                                        Err(e) => println!("{:?}", e),
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
