use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{DataExchange::COPYDATASTRUCT, LibraryLoader::GetModuleHandleW},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::utils;
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

#[derive(Clone)]
pub struct Window {
    pub hwnd: HWND,
    pub controller: ICoreWebView2Controller4,
    pub webview: ICoreWebView2_22,
    pub window_state: WindowState,
    pub widget_wnd: Option<HWND>,
}

impl Window {
    pub fn new(start_mode: &str, args: String) -> (Self, ICoreWebView2Environment) {
        let (hwnd, window_state) = create_window(start_mode);
        let (controller, env, webview) = create_webview2(hwnd, args);
        let widget_wnd = unsafe {
            Some(utils::find_child_window_by_class(
                FindWindowW(w!("krunker_webview"), PCWSTR::null()).unwrap(),
                "Chrome_RenderWidgetHostHWND",
            ))
        };

        let window = Window {
            hwnd,
            controller,
            webview,
            window_state,
            widget_wnd,
        };

        unsafe {
            let window_clone = Box::new(Window {
                hwnd: window.hwnd,
                controller: window.controller.clone(),
                webview: window.webview.clone(),
                window_state: window.window_state,
                widget_wnd: window.widget_wnd,
            });
            SetWindowLongPtrW(window.hwnd, GWLP_USERDATA, Box::into_raw(window_clone) as isize);
        }

        (window, env)
    }
    pub fn toggle_fullscreen(&mut self) {
        unsafe {
            if self.window_state.fullscreen {
                SetWindowLongPtrW(self.hwnd, GWL_STYLE, (WS_VISIBLE.0 | WS_OVERLAPPEDWINDOW.0) as _);

                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    self.window_state.last_position.left,
                    self.window_state.last_position.top,
                    self.window_state.last_position.right - self.window_state.last_position.left,
                    self.window_state.last_position.bottom - self.window_state.last_position.top,
                    SWP_NOZORDER | SWP_FRAMECHANGED,
                )
                .ok();
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
    pub fn handle_accelerator_key(&mut self, key: u16) {
        match VIRTUAL_KEY(key) {
            VK_F4 | VK_F6 => {
                utils::set_cpu_throttling(&self.webview, 1.0);
                unsafe {
                    self.webview.Navigate(w!("https://krunker.io")).ok();
                    // WM_USER with wparam = 0 (unlocked)
                    PostMessageW(self.widget_wnd, WM_USER, WPARAM(0), LPARAM(0)).ok();
                }
            }
            VK_F5 => {
                utils::set_cpu_throttling(&self.webview, 1.0);
                unsafe {
                    self.webview.Reload().ok();
                    // WM_USER with wparam = 0 (unlocked)
                    PostMessageW(self.widget_wnd, WM_USER, WPARAM(0), LPARAM(0)).ok();
                }
            }
            VK_F11 => {
                self.toggle_fullscreen();
            }
            VK_F12 => unsafe {
                self.webview.OpenDevToolsWindow().ok();
            },
            _ => {}
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
) -> (ICoreWebView2Controller4, ICoreWebView2Environment, ICoreWebView2_22) {
    unsafe {
        let args = args + " --autoplay-policy=no-user-gesture-required";
        let options: CoreWebView2EnvironmentOptions = CoreWebView2EnvironmentOptions::default();
        options.set_exclusive_user_data_folder_access(false);
        options.set_are_browser_extensions_enabled(false);
        options.set_additional_browser_arguments(args);
        options.set_language("en-US".to_string());
        options.set_enable_tracking_prevention(false);

        let (tx, rx) = std::sync::mpsc::channel();
        let (etx, erx) = std::sync::mpsc::channel();
        let mut current_exe = std::env::current_exe().unwrap();
        current_exe.pop();

        let result = CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(move |environment_created_handler| {
                CreateCoreWebView2EnvironmentWithOptions(
                    PCWSTR(utils::create_utf_string(current_exe.to_string_lossy() + "\\WebView2").as_ptr()),
                    PCWSTR(
                        utils::create_utf_string(std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp").as_ptr(),
                    ),
                    &ICoreWebView2EnvironmentOptions::from(options),
                    &environment_created_handler,
                )
                .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, env| {
                error_code?;
                let env = env.ok_or_else(|| windows::core::Error::from(E_POINTER)).unwrap();
                let env_clone = env.clone();

                CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
                    Box::new(move |controller_created_handler| {
                        env_clone
                            .CreateCoreWebView2Controller(hwnd, &controller_created_handler)
                            .map_err(webview2_com::Error::WindowsError)
                    }),
                    Box::new(move |controller_error, controller| {
                        controller_error?;
                        let controller = controller.ok_or_else(|| windows::core::Error::from(E_POINTER)).unwrap();

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
                    eprintln!("{}", e);
                    utils::kill("msedgewebview2.exe");
                    let args: Vec<String> = std::env::args().collect();
                    let arg_present = args.iter().any(|arg| arg == "crash");

                    if !arg_present {
                        let current_exe = std::env::current_exe().unwrap();
                        let mut command = std::process::Command::new(&current_exe);
                        command.arg("crash");
                        command.spawn().unwrap().wait().ok();
                    }

                    std::process::exit(0);
                });
                Ok(())
            }),
        );

        if result.is_err() {
            panic!("cannot create webview2 env, {:?}", result)
        };

        let controller = rx.recv().unwrap().cast::<ICoreWebView2Controller4>().unwrap();
        let env = erx.recv().unwrap();
        let webview2 = controller.CoreWebView2().unwrap().cast::<ICoreWebView2_22>().unwrap();

        controller.SetAllowExternalDrop(false).ok();
        controller
            .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                A: 255,
                R: 0,
                G: 0,
                B: 0,
            })
            .ok();

        let webview2_settings = webview2.Settings().unwrap().cast::<ICoreWebView2Settings9>().unwrap();

        webview2_settings.SetIsReputationCheckingRequired(false).ok();
        webview2_settings.SetIsSwipeNavigationEnabled(false).ok();
        webview2_settings.SetIsPinchZoomEnabled(false).ok();
        webview2_settings.SetIsPasswordAutosaveEnabled(false).ok();
        webview2_settings.SetIsGeneralAutofillEnabled(false).ok();
        webview2_settings.SetAreBrowserAcceleratorKeysEnabled(false).ok();
        webview2_settings.SetAreDefaultContextMenusEnabled(false).ok();
        webview2_settings.SetIsZoomControlEnabled(false).ok();
        webview2_settings.SetUserAgent(w!("Electron")).ok();

        // subclass_widgetwin(hwnd);
        (controller, env, webview2)
    }
}

unsafe extern "system" fn wnd_proc_setup(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            #[allow(clippy::all)]
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wnd_proc_main as isize);
            return wnd_proc_main(hwnd, msg, wparam, lparam);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe extern "system" fn wnd_proc_main(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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
                            utils::create_utf_string(format!("window.glorpClient.handleMouseWheel({})", scroll_amount))
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
                window.handle_accelerator_key(wparam.0 as u16);
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
            WM_COPYDATA => {
                let cds_ptr = lparam.0 as *mut COPYDATASTRUCT;
                let cds = &*cds_ptr;
                let data: &[u8] = std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize);

                let string = match String::from_utf8(data.to_vec()) {
                    Ok(s) => s,
                    Err(_) => {
                        eprintln!("Error decoding data from sender.");
                        return LRESULT(0);
                    }
                };
                window
                    .webview
                    .ExecuteScript(
                        PCWSTR(
                            utils::create_utf_string(format!("window.glorpClient.parseArgs('{}')", string)).as_ptr(),
                        ),
                        None,
                    )
                    .ok();
            }
            _ => (),
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
