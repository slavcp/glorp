use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::{
    Win32::{
        Foundation::*, Graphics::Gdi::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*,
    },
    core::*,
};

use crate::{installer, utils};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};

#[derive(Copy, Clone)]
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

pub struct Window {
    pub main: bool,
    pub hwnd: HWND,
    pub controller: ICoreWebView2Controller4,
    pub webview: ICoreWebView2_22,
    pub window_state: WindowState,
}

impl Window {
    pub fn new(start_mode: &str, main: bool) -> (Self, ICoreWebView2Environment) {
        let (hwnd, window_state) = create_window(start_mode);
        let (controller, env, webview) = create_webview2(hwnd, "".to_string());
        let window = Window {
            main,
            hwnd,
            controller,
            webview,
            window_state,
        };

        unsafe {
            let window_clone = Box::new(Window {
                main: window.main,
                hwnd: window.hwnd,
                controller: window.controller.clone(),
                webview: window.webview.clone(),
                window_state: window.window_state,
            });
            SetWindowLongPtrW(
                window.hwnd,
                GWLP_USERDATA,
                Box::into_raw(window_clone) as isize,
            );
        }

        (window, env)
    }
    pub fn toggle_fullscreen(&mut self) {
        unsafe {
            if self.window_state.fullscreen {
                SetWindowLongPtrW(
                    self.hwnd,
                    GWL_STYLE,
                    (WS_VISIBLE.0 | WS_OVERLAPPEDWINDOW.0) as _,
                );

                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    self.window_state.last_position.left,
                    self.window_state.last_position.top,
                    self.window_state.last_position.right - self.window_state.last_position.left,
                    self.window_state.last_position.bottom - self.window_state.last_position.top,
                    SWP_NOZORDER | SWP_FRAMECHANGED,
                )
                .unwrap();
            } else {
                let mut rect = RECT::default();
                let _ = GetWindowRect(self.hwnd, &mut rect);
                self.window_state.last_position = rect;

                SetWindowLongPtrW(self.hwnd, GWL_STYLE, (WS_VISIBLE.0) as _);

                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    0,
                    0,
                    GetSystemMetrics(SYSTEM_METRICS_INDEX(0)),
                    GetSystemMetrics(SYSTEM_METRICS_INDEX(1)),
                    SWP_NOZORDER | SWP_FRAMECHANGED,
                )
                .ok();
            }
            self.window_state.fullscreen = !self.window_state.fullscreen;
        }
    }
}

fn create_window(start_mode: &str) -> (HWND, WindowState) {
    unsafe {
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let icon = match LoadIconW(Some(hinstance), w!("icon")) {
            Ok(icon) => icon,
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap(),
        };

        let class_name = w!("krunker_webview");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc_setup),
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
            "Frameless" => {
                window_style = WS_POPUP | WS_VISIBLE;
            }
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

        let window_state = WindowState {
            fullscreen: fullscreen_state,
            last_position: default_last_position,
        };
        (hwnd, window_state)
    }
}

pub fn create_webview2(
    hwnd: HWND,
    args: String,
) -> (
    ICoreWebView2Controller4,
    ICoreWebView2Environment,
    ICoreWebView2_22,
) {
    unsafe {
        let args = args + " --autoplay-policy=no-user-gesture-required";
        let options = CoreWebView2EnvironmentOptions::default();
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

        // subclass_widgetwin(hwnd);
        (controller, env, webview2)
    }
}

unsafe extern "system" fn wnd_proc_setup(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            #[allow(clippy::all)]
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wnd_proc_main as isize);
                return wnd_proc_main(hwnd, msg, wparam, lparam);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe extern "system" fn wnd_proc_main(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let window_data_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Window;
        if window_data_ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let window = &mut *window_data_ptr;

        match msg {
            WM_MOUSEWHEEL => {
                let delta = utils::HIWORD(wparam.0) as i32;
                let scroll_amount = (delta as f32 / WHEEL_DELTA as f32) * 80.0;

                window
                    .webview
                    .ExecuteScript(
                        PCWSTR(
                            utils::create_utf_string(
                                format!("window.glorpClient.handleMouseWheel({})", scroll_amount)
                                    .as_str(),
                            )
                            .as_ptr(),
                        ),
                        None,
                    )
                    .ok();
            }
            WM_DESTROY => {
                PostQuitMessage(0);
            }
            WM_KEYDOWN => {
                match VIRTUAL_KEY(wparam.0 as u16) {
                    VK_F4 | VK_F6 => {
                        window.webview.Navigate(w!("https://krunker.io")).ok();
                    }
                    VK_F5 => {
                        window.webview.Reload().ok();
                    }
                    VK_F11 => {
                        window.toggle_fullscreen();
                    }
                    VK_F12 => {
                        window.webview.OpenDevToolsWindow().ok();
                    }
                    _ => (),
                };
            }
            WM_SIZE => {
                let bounds = RECT {
                    left: 0,
                    top: 0,
                    right: utils::LOWORD(lparam.0 as usize) as i32,
                    bottom: utils::HIWORD(lparam.0 as usize) as i32,
                };
                window.controller.SetBounds(bounds).ok();
            }
            _ => (),
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
