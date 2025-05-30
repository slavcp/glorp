use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::{
    Win32::{
        Foundation::*, Graphics::Gdi::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*,
    },
    core::*,
};

use crate::{installer, utils};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};

use once_cell::sync::Lazy;
use std::sync::{
    Mutex,
    atomic::{AtomicPtr, Ordering},
};

pub struct WindowState {
    pub fullscreen: bool,
    pub last_position: RECT,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            fullscreen: false,
            last_position: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        }
    }
}
static WINDOW_STATE: Lazy<Mutex<WindowState>> = Lazy::new(|| Mutex::new(WindowState::default()));
static CONTROLLER: AtomicPtr<ICoreWebView2Controller4> = AtomicPtr::new(std::ptr::null_mut());
static WEBVIEW: AtomicPtr<ICoreWebView2_22> = AtomicPtr::new(std::ptr::null_mut());

pub fn create_window(start_mode: &str) -> HWND {
    unsafe {
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let icon = match LoadIconW(Some(hinstance), w!("icon")) {
            Ok(icon) => icon,
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap(),
        };

        let class_name = w!("krunker_webview");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: icon,
            hCursor: Default::default(),
            hbrBackground: CreateSolidBrush(COLORREF(0x00000000)),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: class_name,
        };

        RegisterClassW(&wc);

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let normal_width = (screen_width as f32 * 0.85) as i32;
        let normal_height = (screen_height as f32 * 0.85) as i32;
        let normal_x = (screen_width - normal_width) / 2;
        let normal_y = (screen_height - normal_height) / 2;

        let default_last_position = RECT {
            left: normal_x,
            top: normal_y,
            right: normal_x + normal_width,
            bottom: normal_y + normal_height,
        };

        let window_style;
        let window_ex_style = WINDOW_EX_STYLE::default();
        let mut x = normal_x;
        let mut y = normal_y;
        let mut width = normal_width;
        let mut height = normal_height;
        let mut fullscreen_state = false;

        match start_mode {
            "Borderless Fullscreen" => {
                window_style = WS_VISIBLE;
                x = 0;
                y = 0;
                width = screen_width;
                height = screen_height;
                fullscreen_state = true;
            }
            "Maximized" => {
                window_style = WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_MAXIMIZE;
                x = 0;
                y = 0;
                width = screen_width;
                height = screen_height;
            }
            _ => {
                window_style = WS_OVERLAPPEDWINDOW | WS_VISIBLE;
            }
        }

        if let Ok(mut window_props) = WINDOW_STATE.lock() {
            *window_props = WindowState {
                fullscreen: fullscreen_state,
                last_position: default_last_position,
            };
        }

        let hwnd: HWND = CreateWindowExW(
            window_ex_style,
            class_name,
            w!("glorp"),
            window_style,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(hinstance),
            None,
        )
        .unwrap();
        if start_mode == "Borderless Fullscreen" {
            SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_VISIBLE.0) as _);
        }
        hwnd
    }
}

pub fn create_webview2(
    hwnd: HWND,
    args: String,
) -> (ICoreWebView2Controller4, ICoreWebView2Environment) {
    unsafe {
        let options: CoreWebView2EnvironmentOptions = CoreWebView2EnvironmentOptions::default();
        options.set_are_browser_extensions_enabled(false);
        options.set_additional_browser_arguments(args.clone());
        options.set_language("en-US".to_string());
        options.set_enable_tracking_prevention(false);

        let (tx, rx) = std::sync::mpsc::channel();
        let (etx, erx) = std::sync::mpsc::channel();

        let result = CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(move |environment_created_handler| {
                CreateCoreWebView2EnvironmentWithOptions(
                    PCWSTR::null(),
                    PCWSTR(
                        utils::create_utf_string(
                            &(std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp"),
                        )
                        .as_ptr(),
                    ),
                    &ICoreWebView2EnvironmentOptions::from(options),
                    &environment_created_handler,
                )
                .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, env| {
                error_code?;
                let env = env
                    .ok_or_else(|| windows::core::Error::from(E_POINTER))
                    .unwrap();
                let env_clone = env.clone();

                CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
                    Box::new(move |controller_created_handler| {
                        env_clone
                            .CreateCoreWebView2Controller(hwnd, &controller_created_handler)
                            .map_err(webview2_com::Error::WindowsError)
                    }),
                    Box::new(move |controller_error, controller| {
                        controller_error?;
                        let controller = controller
                            .ok_or_else(|| windows::core::Error::from(E_POINTER))
                            .unwrap();

                        // initial bounds
                        let mut rect = RECT::default();
                        GetClientRect(hwnd, &mut rect).ok();
                        controller.SetBounds(rect).ok();

                        CONTROLLER
                            .store(controller.clone().into_raw() as *mut _, Ordering::Relaxed);
                        WEBVIEW.store(
                            controller.CoreWebView2().unwrap().into_raw() as *mut _,
                            Ordering::Relaxed,
                        );

                        tx.send(controller).expect("error sending controller");
                        etx.send(env).expect("error sending env");

                        Ok(())
                    }),
                )
                .unwrap_or_else(|e| {
                    let error_msg = format!("Failed to create WebView2 environment: {:?}", e);
                    MessageBoxW(
                        None,
                        PCWSTR(utils::create_utf_string(&error_msg).as_ptr()),
                        w!("Error"),
                        MB_OK | MB_ICONERROR,
                    );
                    panic!("{}", error_msg);
                });

                Ok(())
            }),
        );
        if result.is_err() {
            installer::check_webview2();
            return create_webview2(hwnd, args);
        };

        let controller = rx
            .recv()
            .unwrap()
            .cast::<ICoreWebView2Controller4>()
            .unwrap();
        let env = erx.recv().unwrap();
        let webview2 = controller
            .CoreWebView2()
            .unwrap()
            .cast::<ICoreWebView2_22>()
            .unwrap();

        controller.SetAllowExternalDrop(false).unwrap();
        controller
            .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                A: 255,
                R: 0,
                G: 0,
                B: 0,
            })
            .ok();

        let webview2_settings = webview2
            .clone()
            .Settings()
            .unwrap()
            .cast::<ICoreWebView2Settings9>()
            .unwrap();

        webview2_settings
            .SetIsReputationCheckingRequired(false)
            .ok();
        webview2_settings.SetIsSwipeNavigationEnabled(false).ok();
        webview2_settings.SetIsPinchZoomEnabled(false).ok();
        webview2_settings.SetIsPasswordAutosaveEnabled(false).ok();
        webview2_settings.SetIsGeneralAutofillEnabled(false).ok();
        webview2_settings
            .SetAreBrowserAcceleratorKeysEnabled(false)
            .ok();
        webview2_settings
            .SetAreDefaultContextMenusEnabled(false)
            .ok();
        webview2_settings.SetIsZoomControlEnabled(false).ok();
        webview2_settings.SetUserAgent(w!("Electron")).ok();

        WEBVIEW.store(Box::into_raw(Box::new(webview2)), Ordering::Relaxed);
        CONTROLLER.store(
            Box::into_raw(Box::new(controller.clone())),
            Ordering::Relaxed,
        );

        // subclass_widgetwin(hwnd);
        (controller, env)
    }
}
/*
 fn subclass_widgetwin(hwnd: HWND) {
 unsafe {
    let child = FindWindowExW(Some(hwnd), None, w!("Chrome_WidgetWin_0"), PCWSTR::null()).unwrap();

    let original_proc = GetWindowLongPtrW(child, GWLP_WNDPROC);
    CHILD_WINDOW_PROC = transmute::<_, WNDPROC>(original_proc);
    SetWindowLongPtrW(child, GWLP_WNDPROC, child_wnd_proc as _);
    }
}
*/
pub fn toggle_fullscreen(hwnd: HWND) {
    unsafe {
        let mut window_state = WINDOW_STATE.lock().unwrap();
        if window_state.fullscreen {
            SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_VISIBLE.0 | WS_OVERLAPPEDWINDOW.0) as _);

            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                window_state.last_position.left,
                window_state.last_position.top,
                window_state.last_position.right - window_state.last_position.left,
                window_state.last_position.bottom - window_state.last_position.top,
                SWP_NOZORDER | SWP_FRAMECHANGED,
            )
            .unwrap();
        } else {
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            window_state.last_position = rect;

            SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_VISIBLE.0) as _);

            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                0,
                0,
                GetSystemMetrics(SYSTEM_METRICS_INDEX(0)),
                GetSystemMetrics(SYSTEM_METRICS_INDEX(1)),
                SWP_NOZORDER | SWP_FRAMECHANGED,
            )
            .ok();
        }
        window_state.fullscreen = !window_state.fullscreen;
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_MOUSEWHEEL => {
                let webview_ptr = WEBVIEW.load(Ordering::Relaxed);
                if !webview_ptr.is_null() {
                    let webview = &*webview_ptr;
                    let delta = utils::HIWORD(wparam.0) as i32;
                    let scroll_amount = (delta as f32 / WHEEL_DELTA as f32) * 80.0;

                    webview
                        .ExecuteScript(
                            PCWSTR(
                                utils::create_utf_string(
                                    format!(
                                        "window.glorpClient.handleMouseWheel({})",
                                        scroll_amount
                                    )
                                    .as_str(),
                                )
                                .as_ptr(),
                            ),
                            None,
                        )
                        .ok();
                }
            }
            WM_DESTROY => {
                PostQuitMessage(0);
            }
            WM_KEYDOWN => {
                let webview_ptr = WEBVIEW.load(Ordering::Relaxed);
                if !webview_ptr.is_null() {
                    let webview = &*webview_ptr;
                    match VIRTUAL_KEY(wparam.0 as u16) {
                        VK_F4 | VK_F6 => {
                            webview.Navigate(w!("https://krunker.io")).ok();
                        }
                        VK_F5 => {
                            webview.Reload().ok();
                        }
                        VK_F11 => {
                            toggle_fullscreen(hwnd);
                        }
                        VK_F12 => {
                            webview.OpenDevToolsWindow().ok();
                        }
                        _ => (),
                    };
                };
            }
            WM_SIZE => {
                let bounds = RECT {
                    left: 0,
                    top: 0,
                    right: utils::LOWORD(lparam.0 as usize) as i32,
                    bottom: utils::HIWORD(lparam.0 as usize) as i32,
                };
                let controller_ptr = CONTROLLER.load(Ordering::Relaxed);
                if !controller_ptr.is_null() {
                    let controller: &ICoreWebView2Controller = &*controller_ptr;
                    controller.SetBounds(bounds).ok();
                }
            }
            _ => (),
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
