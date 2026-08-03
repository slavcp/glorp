use crate::utils::{config_bool, config_string};
use crate::{constants, handlers, modules, utils, window};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use std::{
    env, fs, io, path, result,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::Win32::Foundation::*;
use windows::Win32::System::Memory::*;
use windows::core::*;

pub fn init_fs() -> result::Result<(), io::Error> {
    let user_profile = path::PathBuf::from(env::var("USERPROFILE").unwrap());
    let client_dir = user_profile.join("Documents").join("glorp");
    let swap_dir = client_dir.join("swapper");
    let scripts_dir = client_dir.join("scripts").join("social");
    let flaglist_path = client_dir.join("user_flags.json");
    let blocklist_path = client_dir.join("user_blocklist.json");

    let resources_dir = env::current_exe().unwrap().parent().unwrap().join("resources");

    fs::create_dir_all(&swap_dir)?;
    fs::create_dir_all(&scripts_dir)?;
    fs::create_dir_all(&resources_dir)?;

    if !path::Path::new(&flaglist_path).exists() {
        fs::write(&flaglist_path, constants::DEFAULT_FLAGS)?;
    }
    if !path::Path::new(&blocklist_path).exists() {
        fs::write(&blocklist_path, constants::DEFAULT_BLOCKLIST)?;
    }
    Ok(())
}

#[repr(C)]
pub(crate) struct SharedStats {
    pub(crate) frame_ns: u64,
    pub(crate) fps: u64,
    pub(crate) target_fps: u64,
}

pub(crate) static SHARED_STATS_PTR: AtomicU64 = AtomicU64::new(0);

pub fn create_main_window(env: Option<ICoreWebView2Environment>) -> window::Window {
    let mut webview2_folder: path::PathBuf = env::current_exe().unwrap();
    webview2_folder.pop();
    webview2_folder = webview2_folder.join("WebView2");

    let fps_limit = crate::CONFIG.lock().unwrap().get::<u64>("fpsLimit").unwrap_or(0);
    unsafe {
        if let Ok(mapping) = CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            None,
            PAGE_READWRITE,
            0,
            24,
            w!("GlorpFrameTiming"),
        ) {
            let view = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, 24);
            if !view.Value.is_null() {
                std::ptr::write_bytes(view.Value as *mut u8, 0, 24);
                (*(view.Value as *mut SharedStats)).target_fps = fps_limit;
                SHARED_STATS_PTR.store(view.Value as u64, Ordering::SeqCst);
            }
        }
    }

    let mut args = modules::flaglist::load();
    if config_bool("uncapFps", true) {
        args.push_str(" --disable-frame-rate-limit");
    }

    let start_mode = config_string("startMode", "Remember Previous");
    let state = if start_mode == "Remember Previous" {
        crate::CONFIG.lock().unwrap().get::<window::WindowState>("lastPosition")
    } else {
        None
    };

    if config_bool("hardFlip", true) {
        fs::rename(
            webview2_folder.join("OLD_vk_swiftshader.dll"),
            webview2_folder.join("vk_swiftshader.dll"),
        )
        .ok();
    } else {
        fs::rename(
            webview2_folder.join("vk_swiftshader.dll"),
            webview2_folder.join("OLD_vk_swiftshader.dll"),
        )
        .ok();
    }

    let main_window = window::Window::new_core(&start_mode, args, env, state);
    let discord_client: Arc<Mutex<Option<DiscordIpcClient>>> = Arc::new(Mutex::new(None));
    if config_bool("discordRPC", true) {
        let mut client = DiscordIpcClient::new(constants::DISCORD_CLIENT_ID);
        client.connect().ok();
        *discord_client.lock().unwrap() = Some(client);
    }

    modules::priority::set(config_string("webviewPriority", "Normal").as_str());

    if config_bool("userscripts", true)
        && let Err(e) = modules::userscripts::load(&main_window.webview, false)
    {
        eprintln!("Failed to load userscripts: {}", e);
    }

    let main_window_ = main_window.clone();
    let js_bundle = {
        #[allow(unused)]
        let mut buf = String::new();
        #[cfg(feature = "editor-ignore")]
        {
            buf = include_str!("../target/bundle.js").to_string();
        }

        #[cfg(feature = "auto-update")]
        if let Ok(buffer) = modules::lifecycle::read_js_bundle() {
            buf = buffer;
        }

        buf
    };

    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(move |_, id| {
        *crate::SCRIPT_ID.lock().unwrap() = id;
        Ok(())
    }));
    unsafe {
        main_window
            .webview
            .AddScriptToExecuteOnDocumentCreated(PCWSTR(utils::create_utf_string(js_bundle).as_ptr()), &handler)
            .ok();
    }

    handlers::set_handlers(&main_window.webview, &main_window.env);

    unsafe {
        main_window
            .webview
            .AddWebResourceRequestedFilter(
                PCWSTR(utils::create_utf_string("*://matchmaker.krunker.io/game-info*").as_ptr()),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            )
            .ok();
    }

    if config_bool("realPing", false) {
        modules::ping::load(&main_window.webview);
    }

    let main_window_for_message = main_window.clone();
    let discord_client_for_message = discord_client.clone();
    let mut web_message_token = 0i64;
    let web_message_handler = WebMessageReceivedEventHandler::create(Box::new(move |webview, args| {
        let Some(webview) = webview else {
            return Ok(());
        };
        let Some(args) = args else {
            return Ok(());
        };
        let mut message = PWSTR::null();
        unsafe {
            args.TryGetWebMessageAsString(&mut message).ok();
        }
        let message_string = take_pwstr(message);
        handlers::handle_web_message(
            &webview,
            &main_window_for_message,
            &discord_client_for_message,
            &message_string,
        )
    }));
    unsafe {
        main_window
            .webview
            .add_WebMessageReceived(&web_message_handler, &mut web_message_token)
            .ok();
    }

    unsafe {
        main_window.webview.Navigate(w!("https://krunker.io")).ok();
    }

    let mut accelerator_token = 0i64;
    let mut main_window_for_accelerator = main_window.clone();
    let accelerator_handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };

        let mut key_event_kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
        unsafe {
            args.KeyEventKind(&mut key_event_kind)?;
        }
        if key_event_kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
            return Ok(());
        }
        let mut pressed_key: u32 = 0;
        unsafe {
            args.VirtualKey(&mut pressed_key)?;
        }

        main_window_for_accelerator.handle_accelerator_key(pressed_key as u16);
        Ok(())
    }));
    unsafe {
        main_window
            .controller
            .clone()
            .add_AcceleratorKeyPressed(&accelerator_handler, &mut accelerator_token)
            .ok();
    }

    main_window_
}
