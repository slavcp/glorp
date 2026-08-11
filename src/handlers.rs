use crate::{debug_print, modules, utils, utils::config, window};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use std::{
    process, result,
    sync::{Arc, Mutex},
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
    core::*,
};

pub fn parse_web_message_value(value: &str) -> serde_json::Value {
    if let Ok(bool_val) = value.parse::<bool>() {
        serde_json::Value::Bool(bool_val)
    } else if let Ok(int_val) = value.parse::<i64>() {
        serde_json::Value::Number(serde_json::Number::from(int_val))
    } else if let Ok(float_val) = value.parse::<f64>() {
        serde_json::Value::Number(serde_json::Number::from_f64((float_val * 100.0).round() / 100.0).unwrap())
    } else {
        serde_json::Value::String(value.to_string())
    }
}

pub fn set_permission_requested_handler(webview: &ICoreWebView2, token: &mut i64) {
    let handler = PermissionRequestedEventHandler::create(Box::new(move |_, args: Option<ICoreWebView2PermissionRequestedEventArgs>| {
        if let Some(args) = args {
            unsafe {
                args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW).ok();
            }
        }
        Ok(())
    }));
    unsafe {
        webview.add_PermissionRequested(&handler, token).ok();
    }
}

pub fn set_web_resource_requested_handler(webview: &ICoreWebView2, env: &ICoreWebView2Environment, token: &mut i64) {
    let env_clone = env.clone();
    let swaps = if config("swapper", true) {
        modules::swapper::load(webview)
    } else {
        std::collections::HashMap::new()
    };

    let handler = WebResourceRequestedEventHandler::create(Box::new(move |webview, args| {
        let Some(args) = args else {
            return Ok(());
        };
        let request: ICoreWebView2WebResourceRequest = unsafe { args.Request()? };
        let mut uri = PWSTR::null();
        unsafe { request.Uri(&mut uri)? };
        let uri = take_pwstr(uri);

        if uri.contains("krunker.io") {
            if uri.contains("game-info") || uri.contains("lobby-ranked") {
                if let Some(webview) = webview {
                    unsafe {
                        webview.PostWebMessageAsString(w!("game-updated")).ok();
                    }
                }
                return Ok(());
            }

            let filename: &str = uri.split("krunker.io/").nth(1).and_then(|s| s.split('?').next()).unwrap_or("");

            if let Some(stream) = swaps.get(filename) {
                let response = unsafe { env_clone.CreateWebResourceResponse(stream, 200, w!("OK"), w!("Access-Control-Allow-Origin: *"))? };
                unsafe { args.SetResponse(Some(&response))? };
                return Ok(());
            }
        }

        unsafe {
            request.SetUri(PCWSTR::null())?;
        }
        Ok(())
    }));

    unsafe {
        webview.add_WebResourceRequested(&handler, token).ok();
    }
}

pub fn set_new_window_requested_handler(webview: &ICoreWebView2, env: &ICoreWebView2Environment, token: &mut i64) {
    let env_clone = env.clone();
    let handler = NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        let features = unsafe { args.WindowFeatures()? };
        let mut has_position: BOOL = false.into();
        let mut has_size: BOOL = false.into();
        unsafe {
            _ = features.HasPosition(&mut has_position);
            _ = features.HasSize(&mut has_size);
        };
        let mut window_state = None;
        if has_position.as_bool() && has_size.as_bool() {
            let mut left = 0;
            let mut top = 0;
            let mut width = 0;
            let mut height = 0;
            unsafe {
                _ = features.Left(&mut left);
                _ = features.Top(&mut top);
                _ = features.Width(&mut width);
                _ = features.Height(&mut height);
            };

            window_state = Some(window::WindowState {
                fullscreen: false,
                position: window::Position {
                    left: left as i32,
                    top: top as i32,
                    right: left as i32 + width as i32,
                    bottom: top as i32 + height as i32,
                },
            });
        }

        let deferral = unsafe { args.GetDeferral()? };
        unsafe {
            args.SetHandled(true).unwrap();
        }
        let (hwnd, window_state) = window::create_window("Custom", true, window_state);
        let mut uri = PWSTR::null();
        let _ = unsafe { args.Uri(&mut uri) };
        let uri = take_pwstr(uri);
        let args = utils::UnsafeSend::new(args);
        let deferral = utils::UnsafeSend::new(deferral);
        let env_for_creation = env_clone.clone();
        let env_for_handler = utils::UnsafeSend::new(env_clone.clone());
        window::create_core_webview2_controller_async(hwnd, env_for_creation, window_state, move |controller| {
            let controller = controller.unwrap();
            let webview = unsafe { controller.CoreWebView2().unwrap() };
            if uri.contains("krunker.io/social.html")
                && config("userscripts", false)
                && let Err(e) = modules::userscripts::load(&webview, true)
            {
                println!("can't load userscripts on social window {}", e);
            }

            unsafe {
                args.take().SetNewWindow(&webview).unwrap();
            }
            set_handlers(&webview, &env_for_handler);

            unsafe {
                deferral.take().Complete().ok();
            }
        });

        Ok(())
    }));

    unsafe {
        webview.add_NewWindowRequested(&handler, token).ok();
    }
}

pub fn set_handlers<T: utils::EnvironmentRef>(webview: &ICoreWebView2, env_wrapper: &T) {
    let env: &ICoreWebView2Environment = env_wrapper.env_ref();
    let mut token = 0i64;

    set_permission_requested_handler(webview, &mut token);

    if config("blocklist", true) {
        modules::blocklist::load(webview);
    }

    set_web_resource_requested_handler(webview, env, &mut token);
    set_new_window_requested_handler(webview, env, &mut token);
}

pub fn send_info(webview: &ICoreWebView2) {
    let version = env!("CARGO_PKG_VERSION");
    let mut info_map = serde_json::Map::new();
    info_map.insert("settings".to_string(), serde_json::json!(&*crate::CONFIG.lock().unwrap()));
    info_map.insert("version".to_string(), serde_json::Value::String(version.to_string()));

    let launch_args = crate::LAUNCH_ARGS.lock().unwrap();
    if !launch_args.is_empty() {
        info_map.insert("launchArgs".to_string(), serde_json::Value::String(launch_args.join(" ")));
    }
    drop(launch_args);

    let info_json = serde_json::to_string_pretty(&info_map).unwrap();

    let info_str = utils::create_utf_string(info_json);
    unsafe {
        webview.PostWebMessageAsJson(PCWSTR(info_str.as_ptr())).ok();
    }
}

pub fn open_documents_subpath(target: &str) {
    let path_to_open = match target {
        "blocklist" => utils::settings_dir().join("user_blocklist.json"),
        "swapper" => utils::settings_dir().join("swapper"),
        "userscripts" => utils::settings_dir().join("scripts"),
        _ => return,
    };
    process::Command::new("explorer.exe").arg(path_to_open).spawn().ok();
}

pub fn handle_web_message(
    webview: &ICoreWebView2,
    main_window: &window::Window,
    discord_client: &Arc<Mutex<Option<DiscordIpcClient>>>,
    message_string: &str,
) -> result::Result<(), windows::core::Error> {
    let parts: Vec<&str> = message_string.split(", ").map(|s| s.trim()).collect();
    debug_print!("web message: {message_string}");

    match parts.as_slice() {
        ["set-config", setting, value] => {
            crate::CONFIG.lock().unwrap().set(setting, parse_web_message_value(value));

            if *setting == "renderFpsLimit"
                && let Ok(fps_limit) = value.parse::<u64>()
            {
                let ptr = crate::app::SHARED_STATS_PTR.load(std::sync::atomic::Ordering::SeqCst);
                if ptr != 0 {
                    unsafe {
                        (*(ptr as *mut crate::app::SharedStats)).target_fps = fps_limit;
                    }
                }
            }
        }
        ["obs-plugin", value] => {
            let install = value.parse::<bool>().unwrap_or(false);
            modules::obs::set_plugin_installed(webview, install);
            if !install {
                crate::CONFIG.lock().unwrap().set("obsCapturePlugin", false);
            }
        }
        ["get-info"] => {
            send_info(webview);
        }
        ["drag", value] => {
            const ENABLED: usize = 2;
            const DISABLED: usize = 0;
            let value = value.parse::<bool>().unwrap_or(false);
            unsafe {
                PostMessageW(main_window.widget_wnd, WM_USER, WPARAM(if value { DISABLED } else { ENABLED }), LPARAM(0)).ok();
            }
        }
        ["throttle", status] => {
            let setting = if *status == "game" { "throttle" } else { "inMenuThrottle" };
            utils::set_cpu_throttling(webview, config(setting, 1.0));
        }
        ["close"] => unsafe {
            PostQuitMessage(0);
        },
        ["clear-cache"] => unsafe {
            webview.CallDevToolsProtocolMethod(w!("Network.clearBrowserCache"), w!("{}"), None).ok();
            webview
                .CallDevToolsProtocolMethod(w!("Storage.clearDataForOrigin"), w!("{\"origin\": \"*\", \"storageTypes\": \"all\"}"), None)
                .ok();
            webview.Reload().ok();
        },
        ["open", target] => {
            open_documents_subpath(target);
        }
        ["rpc-update", part1, part2] => {
            let state = format!("{} on {}", part1, part2);
            if let Some(client) = &mut *discord_client.lock().unwrap() {
                let activity = activity::Activity::new().details("Krunker").state(&state).assets(activity::Assets::new());
                if let Err(e) = client.set_activity(activity) {
                    eprintln!("Failed to set rpc activity: {}", e);
                }
            }
        }
        ["toggle-rboost", value] => {
            const ENABLED: usize = 1;
            const DISABLED: usize = 3;
            let value = value.parse::<bool>().unwrap_or(false);
            unsafe {
                PostMessageW(
                    Some(utils::find_child_window_by_class(
                        FindWindowW(w!("krunker_webview"), PCWSTR::null()).unwrap(),
                        "Chrome_RenderWidgetHostHWND",
                    )),
                    WM_USER,
                    WPARAM(if value { ENABLED } else { DISABLED }),
                    LPARAM(0),
                )
                .ok();
            }
        }
        ["ping"] => {
            modules::ping::ping(webview);
        }
        _ => {}
    }

    Ok(())
}
