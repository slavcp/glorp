#![cfg_attr(feature = "packaged", windows_subsystem = "windows")]
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{Foundation::*, System::Com::*, UI::WindowsAndMessaging::*},
    core::*,
};

use crate::{modules::userscripts, window::WindowState};

mod config;
mod constants;
mod utils;
mod window;
mod modules {
    pub mod blocklist;
    pub mod flaglist;
    pub mod lifecycle;
    pub mod ping;
    pub mod priority;
    pub mod swapper;
    pub mod userscripts;
}

static LAUNCH_ARGS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(std::env::args().skip(1).collect()));
static CONFIG: LazyLock<Mutex<config::Config>> = LazyLock::new(|| Mutex::new(config::Config::load()));
static JS_VERSION: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new("0.0.0".to_string()));
static SCRIPT_ID: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

static mut TOKEN: *mut i64 = &mut 0i64 as *mut i64;

fn init_fs() -> std::result::Result<(), std::io::Error> {
    let user_profile = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap());
    let client_dir = user_profile.join("Documents").join("glorp");
    let swap_dir = client_dir.join("swapper");
    let scripts_dir = client_dir.join("scripts").join("social");
    let flaglist_path = client_dir.join("user_flags.json");
    let blocklist_path = client_dir.join("user_blocklist.json");

    let resources_dir = std::env::current_exe().unwrap().parent().unwrap().join("resources");

    std::fs::create_dir_all(&swap_dir)?;
    std::fs::create_dir_all(&scripts_dir)?;
    std::fs::create_dir_all(&resources_dir)?;

    if !std::path::Path::new(&flaglist_path).exists() {
        std::fs::write(&flaglist_path, constants::DEFAULT_FLAGS)?;
    }
    if !std::path::Path::new(&blocklist_path).exists() {
        std::fs::write(&blocklist_path, constants::DEFAULT_BLOCKLIST)?;
    }
    Ok(())
}

fn set_handlers<T: utils::EnvironmentRef>(webview: &ICoreWebView2, env_wrapper: &T) {
    let env: &ICoreWebView2Environment = env_wrapper.env_ref();
    unsafe {
        webview
            .add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(
                    move |_, args: Option<ICoreWebView2PermissionRequestedEventArgs>| {
                        args.unwrap().SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW).ok();
                        Ok(())
                    },
                )),
                TOKEN,
            )
            .ok();

        let env_ = env.clone();

        if CONFIG.lock().unwrap().get("blocklist").unwrap_or(true) {
            modules::blocklist::load(webview);
        };
        let mut swaps: HashMap<String, IStream> = HashMap::new();

        if CONFIG.lock().unwrap().get("swapper").unwrap_or(true) {
            swaps = modules::swapper::load(webview)
        };

        webview
            .add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |webview, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let request: ICoreWebView2WebResourceRequest = args.Request()?;
                    let mut uri = PWSTR::null();
                    request.Uri(&mut uri)?;
                    let uri = take_pwstr(uri);

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
                            let response = env_.CreateWebResourceResponse(
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

                    Ok(())
                })),
                TOKEN,
            )
            .ok();

        let env_ = env.clone();

        webview
            .add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let features = args.WindowFeatures()?;
                    let mut has_position: BOOL = false.into();
                    let _ = features.HasPosition(&mut has_position);
                    let mut has_size: BOOL = false.into();
                    let _ = features.HasSize(&mut has_size);
                    let mut window_state = None;
                    if has_position.as_bool() && has_size.as_bool() {
                        let mut left = 0;
                        let mut top = 0;
                        let mut width = 0;
                        let mut height = 0;
                        let _ = features.Left(&mut left);
                        let _ = features.Top(&mut top);
                        let _ = features.Width(&mut width);
                        let _ = features.Height(&mut height);
                        window_state = Some(WindowState {
                            fullscreen: false,
                            position: window::Position {
                                left: left as i32,
                                top: top as i32,
                                right: left as i32 + width as i32,
                                bottom: top as i32 + height as i32,
                            },
                        });
                    }

                    let deferral = args.GetDeferral()?;
                    args.SetHandled(true).unwrap();
                    let (hwnd, window_state) = window::create_window("Custom", true, window_state);
                    let mut uri = PWSTR::null();
                    let _ = args.Uri(&mut uri);
                    let uri = take_pwstr(uri);
                    let args = utils::UnsafeSend::new(args);
                    let deferral = utils::UnsafeSend::new(deferral);
                    // man
                    let env_for_creation = env_.clone();
                    let env_for_handler = utils::UnsafeSend::new(env_.clone());
                    window::create_core_webview2_controller_async(
                        hwnd,
                        env_for_creation,
                        window_state,
                        move |controller| {
                            let controller = controller.unwrap();
                            let webview = controller.CoreWebView2().unwrap();
                            if uri.contains("krunker.io/social.html")
                                && CONFIG.lock().unwrap().get("userscripts").unwrap_or(false)
                                && let Err(e) = userscripts::load(&webview, true)
                            {
                                println!("can't load userscripts on social window {}", e);
                            }

                            args.take().SetNewWindow(&webview).unwrap();
                            set_handlers(&webview, &env_for_handler);

                            deferral.take().Complete().ok();
                        },
                    );

                    Ok(())
                })),
                TOKEN,
            )
            .ok();
    }
}

pub fn create_main_window(env: Option<ICoreWebView2Environment>) -> window::Window {
    let mut webview2_folder: std::path::PathBuf = std::env::current_exe().unwrap();
    webview2_folder.pop();
    webview2_folder = webview2_folder.join("WebView2");

    if CONFIG.lock().unwrap().get("hardFlip").unwrap_or(true) {
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

    let mut args = modules::flaglist::load();
    if CONFIG.lock().unwrap().get("uncapFps").unwrap_or(true) {
        args.push_str(" --disable-frame-rate-limit");
    }

    let start_mode = CONFIG
        .lock()
        .unwrap()
        .get::<String>("startMode")
        .unwrap_or_else(|| String::from("Remember Previous"));

    let state = if start_mode == "Remember Previous" {
        crate::CONFIG.lock().unwrap().get::<window::WindowState>("lastPosition")
    } else {
        None
    };

    let mut main_window = window::Window::new_core(&start_mode, args, env, state);

    let discord_client: Mutex<Option<DiscordIpcClient>> = Mutex::new(None);
    if CONFIG.lock().unwrap().get("discordRPC").unwrap_or(false) {
        let mut client = DiscordIpcClient::new(constants::DISCORD_CLIENT_ID);
        client.connect().ok();
        *discord_client.lock().unwrap() = Some(client);
    }

    modules::priority::set(
        CONFIG
            .lock()
            .unwrap()
            .get::<String>("webviewPriority")
            .unwrap_or(String::from("Normal"))
            .as_str(),
    );

    if CONFIG.lock().unwrap().get("userscripts").unwrap_or(false)
        && let Err(e) = userscripts::load(&main_window.webview, false)
    {
        eprintln!("Failed to load userscripts: {}", e);
    }

    let main_window_ = main_window.clone();

    #[allow(unused_mut)]
    let mut buf = include_str!("../target/bundle.js").to_string();

    #[cfg(feature = "packaged")]
    if let Ok(buffer) = modules::lifecycle::read_js_bundle() {
        buf = buffer;
    }

    // > memory safe language
    // unsafe
    unsafe {
        main_window
            .webview
            .AddScriptToExecuteOnDocumentCreated(
                PCWSTR(utils::create_utf_string(buf).as_ptr()),
                &AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(move |_, id| {
                    *SCRIPT_ID.lock().unwrap() = id;
                    Ok(())
                })),
            )
            .ok();
        set_handlers(&main_window.webview, &main_window.env);

        main_window
            .webview
            .AddWebResourceRequestedFilter(
                PCWSTR(utils::create_utf_string("*://matchmaker.krunker.io/game-info*").as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            )
            .ok();

        if CONFIG.lock().unwrap().get("realPing").unwrap_or(false) {
            modules::ping::load(&main_window.webview);
        }

        if CONFIG.lock().unwrap().get("rampBoost").unwrap_or(false) {
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
            .webview
            .add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |webview, args| {
                    let Some(webview) = webview else {
                        return Ok(());
                    };
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let mut message = PWSTR::null();
                    args.TryGetWebMessageAsString(&mut message).ok();
                    let message_string = take_pwstr(message);
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
                            CONFIG.lock().unwrap().set(setting, value);
                        }
                        Some(&"get-info") => {
                            let version = env!("CARGO_PKG_VERSION");
                            let mut info_map = serde_json::Map::new();
                            info_map.insert("settings".to_string(), serde_json::json!(&*CONFIG.lock().unwrap()));
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
                                    CONFIG.lock().unwrap().get::<f32>("throttle").unwrap_or(1.0),
                                );
                            } else {
                                utils::set_cpu_throttling(
                                    &webview,
                                    CONFIG.lock().unwrap().get::<f32>("inMenuThrottle").unwrap_or(2.0),
                                );
                            }
                        }
                        Some(&"close") => {
                            PostQuitMessage(0);
                        }
                        Some(&"open") => {
                            let client_dir: String = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp";
                            let path_to_open = match parts[1] {
                                "blocklist" => Some(std::path::PathBuf::from(&client_dir).join("user_blocklist.json")),
                                "swapper" => Some(std::path::PathBuf::from(&client_dir).join("swapper")),
                                "userscripts" => Some(std::path::PathBuf::from(&client_dir).join("scripts")),
                                _ => None,
                            };
                            if let Some(path) = path_to_open {
                                std::process::Command::new("explorer.exe").arg(&path).spawn().ok();
                            }
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
                            modules::ping::ping(&webview);
                        }
                        _ => {}
                    }

                    Ok(())
                })),
                TOKEN,
            )
            .ok();

        main_window.webview.Navigate(w!("https://krunker.io")).ok();

        main_window
            .controller
            .clone()
            .add_AcceleratorKeyPressed(
                &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut key_event_kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                    args.KeyEventKind(&mut key_event_kind)?;
                    if key_event_kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
                        return Ok(());
                    }
                    let mut pressed_key: u32 = 0;
                    args.VirtualKey(&mut pressed_key)?;

                    main_window.handle_accelerator_key(pressed_key as u16);
                    Ok(())
                })),
                TOKEN,
            )
            .ok();
    };

    main_window_
}

fn main() {
    modules::lifecycle::register_instance();
    #[cfg(feature = "packaged")]
    {
        modules::lifecycle::set_panic_hook().ok();
        modules::lifecycle::installer_cleanup().ok();
    }

    if let Err(e) = init_fs() {
        eprintln!("failed to set all the files in place {}", e);
    }

    let window = create_main_window(None);
    let (_tx, rx) = std::sync::mpsc::channel::<String>();
    #[cfg(feature = "packaged")]
    {
        let main_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
        if CONFIG.lock().unwrap().get("checkUpdates").unwrap_or(true) {
            std::thread::spawn(move || {
                modules::lifecycle::check_major_update();
                if let Some(new_js) = modules::lifecycle::check_minor_update() {
                    _tx.send(new_js).ok();
                    unsafe {
                        PostThreadMessageW(main_thread_id, constants::WM_MINOR_UPDATE_READY, WPARAM(0), LPARAM(0))
                            .unwrap();
                    }
                }
            });
        }
    }
    let mut msg: MSG = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            if msg.message == constants::WM_MINOR_UPDATE_READY
                && let Ok(js_content) = rx.try_recv()
            {
                println!("updating js, {}", &*SCRIPT_ID.lock().unwrap());
                window
                    .webview
                    .RemoveScriptToExecuteOnDocumentCreated(PCWSTR(
                        utils::create_utf_string(&*SCRIPT_ID.lock().unwrap()).as_ptr(),
                    ))
                    .ok();
                window
                    .webview
                    .AddScriptToExecuteOnDocumentCreated(PCWSTR(utils::create_utf_string(js_content).as_ptr()), None)
                    .ok();
            }
            DispatchMessageW(&msg);
        }
    }
    // code here runs after window is closed

    CONFIG.lock().unwrap().save();
}
