#![cfg_attr(feature = "packaged", windows_subsystem = "windows")]
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex, atomic::*},
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{Foundation::*, System::Com::*, UI::WindowsAndMessaging::*},
    core::*,
};

mod config;
mod constants;
mod utils;
mod window;
mod modules {
    pub mod blocklist;
    pub mod flaglist;
    pub mod lifecycle;
    pub mod priority;
    pub mod swapper;
    pub mod userscripts;
}

static LAUNCH_ARGS: Lazy<Arc<Mutex<Vec<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(std::env::args().skip(1).collect())));

static LAST_CONNECTED_LOBBY: Lazy<Arc<Mutex<IpAddr>>> =
    Lazy::new(|| Arc::new(Mutex::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))));

static PING: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));

fn main() {
    #[cfg(feature = "packaged")]
    {
        modules::lifecycle::set_panic_hook().ok();
        modules::lifecycle::installer_cleanup().ok();
    }
    modules::lifecycle::register_instance();

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
    let webview2_folder: std::path::PathBuf = std::env::current_dir().unwrap().join("WebView2");

    let config = Arc::new(Mutex::new(config::Config::load()));
    if config.lock().unwrap().get("hardFlip").unwrap_or(true) {
        std::fs::rename(
            webview2_folder.join("OLD_vk_swiftshader.dll"),
            webview2_folder.join("vk_swiftshader.dll"),
        )
        .ok();
    } else {
        std::fs::rename(
            webview2_folder.join("vk_swiftshader.dll"),
            webview2_folder.join("OLD_vk_swiftshader.dll"),
        )
        .ok();
    }

    let token: *mut i64 = &mut 0i64 as *mut i64;
    let mut args = modules::flaglist::load();

    if config.lock().unwrap().get("uncapFps").unwrap_or(true) {
        args.push_str("--disable-frame-rate-limit")
    }

    let (main_window, env) = window::Window::new(
        config
            .lock()
            .unwrap()
            .get::<String>("startMode")
            .unwrap_or_else(|| String::from("Borderless Fullscreen"))
            .as_str(),
        args,
    );

    let discord_client: Mutex<Option<DiscordIpcClient>> = Mutex::new(None);
    if config.lock().unwrap().get("discordRPC").unwrap_or(false) {
        let mut client = DiscordIpcClient::new(constants::DISCORD_CLIENT_ID);
        client.connect().ok();
        *discord_client.lock().unwrap() = Some(client);
    }

    modules::priority::set(
        config
            .lock()
            .unwrap()
            .get::<String>("webviewPriority")
            .unwrap_or(String::from("Normal"))
            .as_str(),
    );

    let mut webview_pid: u32 = 0;

    unsafe {
        main_window.webview.BrowserProcessId(&mut webview_pid).ok();
    }

    println!("Webview PID: {}", webview_pid);
    #[cfg(feature = "packaged")]
    {
        if config.lock().unwrap().get("checkUpdates").unwrap_or(false) {
            modules::lifecycle::check_update();
        }
    }

    if config.lock().unwrap().get("userscripts").unwrap_or(false)
        && let Err(e) = modules::userscripts::load(&main_window.webview)
    {
        eprintln!("Failed to load userscripts: {}", e);
    }
    unsafe {
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

        main_window
            .webview
            .add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(
                    move |_, args: Option<ICoreWebView2PermissionRequestedEventArgs>| {
                        args.unwrap().SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW).ok();
                        Ok(())
                    },
                )),
                token,
            )
            .ok();
    }
    let mut swaps: HashMap<String, IStream> = HashMap::new();

    if config.lock().unwrap().get("blocklist").unwrap_or(true) {
        modules::blocklist::load(&main_window.webview);
    };

    if config.lock().unwrap().get("swapper").unwrap_or(true) {
        swaps = modules::swapper::load(&main_window.webview)
    };

    unsafe {
        main_window
            .webview
            .AddWebResourceRequestedFilterWithRequestSourceKinds(
                PCWSTR(utils::create_utf_string("*://matchmaker.krunker.io/game-info*").as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
            )
            .ok();

        main_window
            .webview
            .add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(
                    move |webview: Option<ICoreWebView2>, args: Option<ICoreWebView2WebResourceRequestedEventArgs>| {
                        if let Some(args) = args {
                            let request: ICoreWebView2WebResourceRequest = args.Request()?;
                            let mut uri_string = utils::create_utf_string("");
                            let uri = uri_string.as_mut_ptr() as *mut PWSTR;
                            request.Uri(uri)?;
                            let uri = uri.as_ref().unwrap().to_string()?;
                            if uri.contains("krunker.io") {
                                if uri.contains("game-info") || uri.contains("lobby-ranked") {
                                    webview.unwrap().PostWebMessageAsString(w!("game-updated")).ok();
                                    return Ok(());
                                }
                                let filename: &str = uri
                                    .split("krunker.io/")
                                    .nth(1)
                                    .and_then(|s| s.split('?').next())
                                    .unwrap_or("");

                                let stream = swaps.get(filename);
                                if let Some(stream) = stream {
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
                            // other cases MUST be from the blocklist
                            request.SetUri(PCWSTR::null())?;
                            return Ok(());
                        }
                        Ok(())
                    },
                )),
                token,
            )
            .ok();
    }

    if config.lock().unwrap().get("realPing").unwrap_or(false) {
        unsafe {
            main_window
                .webview
                .CallDevToolsProtocolMethod(w!("Network.enable"), w!("{}"), None)
                .ok();
        }
        let ws_receiver = unsafe {
            main_window
                .webview
                .GetDevToolsProtocolEventReceiver(w!("Network.webSocketCreated"))
                .unwrap()
        };

        let handler = DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                let mut params_vec = utils::create_utf_string("");
                let params = params_vec.as_mut_ptr() as *mut PWSTR;
                unsafe {
                    args.ParameterObjectAsJson(params)?;
                }
                let json = unsafe {
                    serde_json::from_str::<serde_json::Value>(&params.as_ref().unwrap().to_string().unwrap()).unwrap()
                };
                let url = json.get("url").unwrap().to_string();
                if url.contains("lobby-") {
                    let host = url.split("://").last().unwrap().split("/").next().unwrap().to_string();
                    let resolved_ips = dns_lookup::lookup_host(&host)?;
                    if let Some(ip) = resolved_ips.into_iter().next() {
                        *LAST_CONNECTED_LOBBY.lock().unwrap() = ip;
                    }
                }
            }
            Ok(())
        }));

        unsafe {
            ws_receiver.add_DevToolsProtocolEventReceived(&handler, token).ok();
        }
        std::thread::spawn(move || {
            loop {
                let result = ping_rs::send_ping(
                    &LAST_CONNECTED_LOBBY.lock().unwrap(),
                    std::time::Duration::from_secs(1),
                    Default::default(),
                    Some(&ping_rs::PingOptions {
                        ttl: 128,
                        dont_fragment: true,
                    }),
                );
                if let Ok(reply) = result {
                    PING.store(reply.rtt, Ordering::Relaxed);
                }
                std::thread::sleep(std::time::Duration::from_millis(3000));
            }
        });
    }

    let config_clone = Arc::clone(&config);

    unsafe {
        main_window
            .webview
            .add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(
                    move |webview, args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
                        let webview = webview.unwrap();
                        let args = args.unwrap();
                        let mut message_vec = utils::create_utf_string("");
                        let message = message_vec.as_mut_ptr() as *mut PWSTR;
                        args.TryGetWebMessageAsString(message).ok();
                        let message_string = message.as_ref().unwrap().to_string().unwrap();

                        let parts: Vec<&str> = message_string.split(", ").map(|s| s.trim()).collect();
                        match parts.first() {
                            Some(&"set-config") => {
                                let setting = parts[1];
                                let value = if let Ok(bool_val) = parts[2].parse::<bool>() {
                                    serde_json::Value::Bool(bool_val)
                                } else if let Ok(int_val) = parts[2].parse::<i64>() {
                                    serde_json::Value::Number(serde_json::Number::from(int_val))
                                } else if let Ok(float_val) = parts[2].parse::<f64>() {
                                    serde_json::Value::Number(
                                        serde_json::Number::from_f64((float_val * 100.0).round() / 100.0).unwrap(),
                                    )
                                } else {
                                    serde_json::Value::String(parts[2].to_string())
                                };
                                config_clone.lock().unwrap().set(setting, value);
                            }
                            Some(&"get-info") => {
                                let version = env!("CARGO_PKG_VERSION");
                                let mut info_map = serde_json::Map::new();
                                info_map.insert("settings".to_string(), serde_json::json!(&*config_clone));
                                info_map.insert("version".to_string(), serde_json::Value::String(version.to_string()));
                                if !LAUNCH_ARGS.lock().unwrap().is_empty() {
                                    info_map.insert(
                                        "launchArgs".to_string(),
                                        serde_json::Value::String(LAUNCH_ARGS.lock().unwrap().join(" ")),
                                    );
                                }

                                let info_json = serde_json::to_string_pretty(&info_map).unwrap();

                                webview
                                    .PostWebMessageAsJson(PCWSTR(utils::create_utf_string(info_json).as_ptr()))
                                    .ok();
                            }
                            Some(&"pointer-lock") => {
                                let value = parts[1].parse::<bool>().unwrap_or(false);
                                // WM_USER with wparam = 0 (unlocked) or 2 (locked)
                                PostMessageW(
                                    main_window.widget_wnd,
                                    WM_USER,
                                    WPARAM(if value { 2 } else { 0 }),
                                    LPARAM(0),
                                )
                                .ok();
                                if value {
                                    utils::set_cpu_throttling(
                                        &webview,
                                        config_clone.lock().unwrap().get::<f32>("throttle").unwrap_or(1.0),
                                    );
                                } else {
                                    utils::set_cpu_throttling(
                                        &webview,
                                        config_clone.lock().unwrap().get::<f32>("inMenuThrottle").unwrap_or(2.0),
                                    );
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
                            Some(&"rpc-update") => {
                                let state = format!("{} on {}", parts[1], parts[2]);
                                if let Some(client) = &mut *discord_client.lock().unwrap() {
                                    let activity = activity::Activity::new()
                                        .details("Krunker")
                                        .state(&state)
                                        .assets(activity::Assets::new());

                                    if let Err(e) = client.set_activity(activity) {
                                        eprintln!("Failed to set rpc activity: {}", e);
                                    }
                                }
                            }
                            Some(&"ping") => {
                                webview
                                    .PostWebMessageAsJson(PCWSTR(
                                        utils::create_utf_string(format!(
                                            "{{\"pingInfo\":{}}}",
                                            &PING.load(Ordering::Relaxed)
                                        ))
                                        .as_ptr(),
                                    ))
                                    .ok();
                            }
                            _ => {}
                        }

                        Ok(())
                    },
                )),
                token,
            )
            .ok();

        if config.lock().unwrap().get("rampBoost").unwrap_or(false) {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(6000));
                PostMessageW(
                    Some(utils::find_child_window_by_class(
                        FindWindowW(w!("krunker_webview"), PCWSTR::null()).unwrap(),
                        "Chrome_RenderWidgetHostHWND",
                    )),
                    WM_USER,
                    WPARAM(1),
                    LPARAM(0),
                )
                .ok();
            });
        }

        main_window
            .controller
            .clone()
            .add_AcceleratorKeyPressed(
                &AcceleratorKeyPressedEventHandler::create(Box::new(
                    move |_, args: Option<ICoreWebView2AcceleratorKeyPressedEventArgs>| {
                        let mut pressed_key: u32 = 0;
                        let mut key_event_kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                        let args = args.unwrap();

                        args.KeyEventKind(&mut key_event_kind)?;
                        args.VirtualKey(&mut pressed_key)?;
                        if key_event_kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
                            return Ok(());
                        }
                        main_window.clone().handle_accelerator_key(pressed_key as u16);
                        Ok(())
                    },
                )),
                token,
            )
            .ok();
    }

    let mut msg: MSG = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    // code here runs after window is closed

    config.lock().unwrap().save();
}
