#![cfg_attr(feature = "packaged", windows_subsystem = "windows")]
use std::{
    env, sync,
    sync::{LazyLock, Mutex, atomic::Ordering},
};
use windows::{Win32::UI::WindowsAndMessaging::*, core::*};

mod app;
mod config;
mod constants;
mod handlers;
mod utils;
mod window;
pub mod modules {
    pub mod blocklist;
    pub mod flaglist;
    pub mod lifecycle;
    pub mod ping;
    pub mod priority;
    pub mod swapper;
    pub mod userscripts;
}

static LAUNCH_ARGS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(env::args().skip(1).collect()));
static CONFIG: LazyLock<Mutex<config::Config>> = LazyLock::new(|| Mutex::new(config::Config::load()));
static JS_VERSION: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new("0.0.0".to_string()));
static SCRIPT_ID: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

fn main() {
    modules::lifecycle::register_instance();
    #[cfg(feature = "packaged")]
    {
        modules::lifecycle::set_panic_hook().ok();
        modules::lifecycle::installer_cleanup().ok();
    }

    if let Err(e) = app::init_fs() {
        eprintln!("failed to set all the files in place {}", e);
    }

    let window = app::create_main_window(None);
    let (_tx, rx) = sync::mpsc::channel::<String>();
    #[cfg(feature = "auto-update")]
    {
        use utils::config_bool;
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        let main_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
        if config_bool("checkUpdates", true) {
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
    if utils::config_bool("renderStats", false) {
        unsafe {
            SetTimer(None, 1, 100, None);
        }
    }
    let mut last_render_stats: Option<(u64, u64)> = None;
    let mut msg: MSG = MSG::default();
    loop {
        let has_msg = unsafe { GetMessageW(&mut msg, None, 0, 0).as_bool() };
        if !has_msg {
            break;
        }
        unsafe {
            _ = TranslateMessage(&msg);
        }
        if msg.message == constants::WM_MINOR_UPDATE_READY
            && let Ok(js_content) = rx.try_recv()
        {
            println!("updating js, {}", *SCRIPT_ID.lock().unwrap());
            unsafe {
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
        }
        if msg.message == WM_TIMER {
            let ptr = app::SHARED_STATS_PTR.load(Ordering::SeqCst);
            if ptr != 0 {
                let (frame_ns, fps) = unsafe {
                    let shared = &*(ptr as *const app::SharedStats);
                    (shared.frame_ns, shared.fps)
                };

                // Current state tracks (fps, frame_ns)
                let current = (fps, frame_ns);

                if fps > 0 && last_render_stats != Some(current) {
                    last_render_stats = Some(current);

                    println!("render stats: fps={}", fps);

                    let payload = format!("{{\"fpsInfo\":{}}}", fps);
                    unsafe {
                        window
                            .webview
                            .PostWebMessageAsJson(PCWSTR(utils::create_utf_string(payload).as_ptr()))
                            .ok();
                    }
                }
            }
        }
        unsafe {
            DispatchMessageW(&msg);
        }
    }
    // code here runs after window is closed

    CONFIG.lock().unwrap().save();
}
