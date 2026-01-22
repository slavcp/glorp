use crate::create_main_window;
use crate::utils;
use std::sync::atomic::{AtomicUsize, Ordering};
use webview2_com::{Error, Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{DataExchange::COPYDATASTRUCT, LibraryLoader::GetModuleHandleW},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::*,
};

static WINDOW_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Copy, Clone, serde::Serialize, serde::Deserialize, Default, Debug)]
pub struct Position {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<RECT> for Position {
    fn from(rect: RECT) -> Self {
        Position {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

#[derive(Copy, Clone, serde::Serialize, serde::Deserialize, Default, Debug)]
pub struct WindowState {
    pub fullscreen: bool,
    pub position: Position,
}

#[derive(Clone)]
pub struct Window {
    pub hwnd: HWND,
    pub env: ICoreWebView2Environment,
    pub controller: ICoreWebView2Controller,
    pub webview: ICoreWebView2,
    pub state: WindowState,
    pub widget_wnd: Option<HWND>,
}

impl Window {
    pub fn new_core(
        start_mode: &str,
        args: String,
        env: Option<ICoreWebView2Environment>,
        state: Option<WindowState>,
    ) -> Self {
        let (hwnd, state) = create_window(start_mode, false, state);
        let (controller, env, webview) = create_webview2(hwnd, args, env);
        let widget_wnd = unsafe {
            Some(utils::find_child_window_by_class(
                FindWindowW(w!("krunker_webview"), PCWSTR::null()).unwrap(),
                "Chrome_RenderWidgetHostHWND",
            ))
        };
        let window = Window {
            hwnd,
            env,
            controller,
            webview,
            state,
            widget_wnd,
        };

        unsafe {
            let window_clone = Box::new(Window {
                hwnd: window.hwnd,
                env: window.env.clone(),
                controller: window.controller.clone(),
                webview: window.webview.clone(),
                state,
                widget_wnd: window.widget_wnd,
            });
            SetWindowLongPtrW(window.hwnd, GWLP_USERDATA, Box::into_raw(window_clone) as isize);
        }

        window
    }
    pub fn toggle_fullscreen(&mut self) {
        unsafe {
            if self.state.fullscreen {
                SetWindowLongPtrW(self.hwnd, GWL_STYLE, (WS_VISIBLE.0 | WS_OVERLAPPEDWINDOW.0) as _);

                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    self.state.position.left,
                    self.state.position.top,
                    self.state.position.right - self.state.position.left,
                    self.state.position.bottom - self.state.position.top,
                    SWP_NOZORDER | SWP_FRAMECHANGED,
                )
                .ok();
            } else {
                let mut rect = RECT::default();
                GetWindowRect(self.hwnd, &mut rect).ok();
                self.state.position = Position::from(rect);

                SetWindowLongPtrW(self.hwnd, GWL_STYLE, (WS_VISIBLE.0) as _);

                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    0,
                    0,
                    GetSystemMetrics(SM_CXSCREEN),
                    GetSystemMetrics(SM_CYSCREEN),
                    SWP_NOZORDER | SWP_FRAMECHANGED,
                )
                .ok();
            }
            self.state.fullscreen = !self.state.fullscreen;
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

pub fn create_window(start_mode: &str, is_subwindow: bool, init_state: Option<WindowState>) -> (HWND, WindowState) {
    unsafe {
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let icon = match LoadIconW(Some(hinstance), w!("icon")) {
            Ok(icon) => icon,
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap(),
        };
        let class_name = if is_subwindow {
            w!("krunker_webview_subwindow")
        } else {
            w!("krunker_webview")
        };
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

        fn windowed_size(state: &mut WindowState) {
            let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
            let window_width = (screen_width as f32 * 0.8) as i32;
            let window_height = (screen_height as f32 * 0.8) as i32;
            let left = (screen_width - window_width) / 2;
            let top = (screen_height - window_height) / 2;
            state.fullscreen = false;
            state.position = Position {
                left,
                top,
                right: left + window_width,
                bottom: top + window_height,
            };
        }

        let state: WindowState = {
            //fallback
            let mut creation_state = WindowState {
                fullscreen: true,
                position: Position {
                    left: 0,
                    top: 0,
                    right: screen_width,
                    bottom: screen_height,
                },
            };
            match start_mode {
                "Borderless Fullscreen" => {}
                "Maximized" => {
                    creation_state.fullscreen = false;
                }
                "Remember Previous" => {
                    if let Some(init_state) = init_state {
                        creation_state = init_state;
                    }
                }
                "Custom" => {
                    if let Some(init_state) = init_state {
                        creation_state = init_state;
                    } else {
                        windowed_size(&mut creation_state);
                    }
                }
                _ => {
                    windowed_size(&mut creation_state);
                }
            }
            creation_state
        };

        let hwnd: HWND = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("glorp"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            state.position.left,
            state.position.top,
            state.position.right - state.position.left,
            state.position.bottom - state.position.top,
            None,
            None,
            Some(hinstance),
            Some((is_subwindow as isize) as *mut std::ffi::c_void),
        )
        .unwrap();

        if state.fullscreen {
            SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_VISIBLE.0) as _);
        }

        (hwnd, state)
    }
}

pub fn create_core_webview2_controller_async<F>(
    hwnd: HWND,
    env: ICoreWebView2Environment,
    state: WindowState,
    callback: F,
) where
    F: FnOnce(std::result::Result<ICoreWebView2Controller, Error>) + Send + 'static,
{
    let env_ = env.clone();
    let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(move |_, controller| {
        if let Some(controller) = controller {
            unsafe {
                let webview = controller.CoreWebView2().unwrap();
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect).ok();
                controller.SetBounds(rect).ok();
                let window = Box::new(Window {
                    hwnd,
                    env: env_,
                    controller: controller.clone(),
                    webview: webview.clone(),
                    state,
                    widget_wnd: None,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(window) as isize);
            }
            callback(Ok(controller));
        }
        Ok(())
    }));

    unsafe {
        if let Err(err) = env.CreateCoreWebView2Controller(hwnd, &handler) {
            eprintln!("can't create CoreWebView2Controller: {}", err);
        }
    }
}

pub fn create_webview2(
    hwnd: HWND,
    args: String,
    provided_env: Option<ICoreWebView2Environment>,
) -> (ICoreWebView2Controller, ICoreWebView2Environment, ICoreWebView2) {
    unsafe {
        let options: CoreWebView2EnvironmentOptions = CoreWebView2EnvironmentOptions::default();
        options.set_exclusive_user_data_folder_access(false);
        options.set_are_browser_extensions_enabled(false);
        options.set_additional_browser_arguments(args);
        options.set_language("en-US".to_string());
        options.set_enable_tracking_prevention(false);
        let env = if let Some(provided_env) = provided_env {
            provided_env
        } else {
            let (etx, erx) = std::sync::mpsc::channel();
            let mut current_dir = std::env::current_exe().unwrap();
            current_dir.pop();
            let result = CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
                Box::new(move |environment_created_handler| {
                    CreateCoreWebView2EnvironmentWithOptions(
                        PCWSTR(utils::create_utf_string(current_dir.to_string_lossy() + "\\\\WebView2").as_ptr()),
                        PCWSTR(
                            utils::create_utf_string(std::env::var("USERPROFILE").unwrap() + "\\\\Documents\\\\glorp")
                                .as_ptr(),
                        ),
                        &ICoreWebView2EnvironmentOptions::from(options),
                        &environment_created_handler,
                    )
                    .map_err(Error::WindowsError)
                }),
                Box::new(move |error_code, env| {
                    error_code?;
                    let env = env.ok_or_else(|| Error::from(E_POINTER)).unwrap();
                    etx.send(env).expect("error sending env");
                    Ok(())
                }),
            );

            if result.is_err() {
                panic!("cannot create webview2 env, {:?}", result)
            };

            erx.recv().unwrap()
        };

        let env_ = env.clone();
        let controller = {
            let (tx, rx) = std::sync::mpsc::channel();

            CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
                Box::new(move |handler| {
                    env.CreateCoreWebView2Controller(hwnd, &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error, controller| {
                    error?;
                    let controller = controller.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                    let mut rect = RECT::default();
                    GetClientRect(hwnd, &mut rect).ok();
                    controller.SetBounds(rect).ok();

                    tx.send(controller).unwrap();
                    Ok(())
                }),
            )
            .unwrap_or_else(|e| {
                eprintln!("crash {}", e);
                utils::kill("msedgewebview2.exe");
                let args: Vec<String> = std::env::args().collect();
                let arg_present = args.iter().any(|arg| arg == "crash");

                if !arg_present {
                    let current_exe = std::env::current_exe().unwrap();
                    let mut command = std::process::Command::new(&current_exe);
                    command.arg("crash");
                    command.spawn().ok();
                }

                std::process::exit(0);
            });
            rx.recv().unwrap()
        };
        let webview2 = controller.CoreWebView2().unwrap();

        set_wv_settings(&webview2, &controller);

        // subclass_widgetwin(hwnd);
        (controller, env_, webview2)
    }
}

pub fn set_wv_settings(webview: &ICoreWebView2, controller: &ICoreWebView2Controller) {
    unsafe {
        let controller = controller.cast::<ICoreWebView2Controller4>().unwrap();

        controller.SetAllowExternalDrop(false).ok();
        controller
            .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                A: 255,
                R: 0,
                G: 0,
                B: 0,
            })
            .ok();
        let webview2_settings = webview.Settings().unwrap().cast::<ICoreWebView2Settings9>().unwrap();

        let _ = webview2_settings.SetIsReputationCheckingRequired(false);
        let _ = webview2_settings.SetIsSwipeNavigationEnabled(false);
        let _ = webview2_settings.SetIsPinchZoomEnabled(false);
        let _ = webview2_settings.SetIsPasswordAutosaveEnabled(false);
        let _ = webview2_settings.SetIsGeneralAutofillEnabled(false);
        let _ = webview2_settings.SetAreBrowserAcceleratorKeysEnabled(false);
        let _ = webview2_settings.SetAreDefaultContextMenusEnabled(false);
        let _ = webview2_settings.SetIsZoomControlEnabled(false);
        let _ = webview2_settings.SetUserAgent(w!("Electron"));
    }
}

unsafe extern "system" fn wnd_proc_setup(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let create_struct = lparam.0 as *const CREATESTRUCTW;
            let is_subwindow = (*create_struct).lpCreateParams as isize;
            WINDOW_COUNT.fetch_add(1, Ordering::SeqCst);
            #[allow(clippy::fn_to_numeric_cast)]
            let wnd_proc = if is_subwindow == 0 {
                wnd_proc_main as isize
            } else {
                wnd_proc_subwindow as isize
            };

            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wnd_proc);
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
            WM_SETFOCUS => {
                let child = GetWindow(hwnd, GW_CHILD).ok();
                if child.is_some() {
                    SetFocus(child).ok();
                }
            }
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
                window.controller.Close().ok();
                drop(Box::from_raw(window_data_ptr));
                let count = WINDOW_COUNT.fetch_sub(1, Ordering::SeqCst);

                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect).ok();
                window.state.position = Position::from(rect);
                let styles = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                window.state.fullscreen = (styles & WS_OVERLAPPEDWINDOW.0) == 0;

                crate::CONFIG.lock().unwrap().set("lastPosition", window.state);

                if count == 1 {
                    PostQuitMessage(0);
                }
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
                if let Ok(mut string) = String::from_utf8(data.to_vec()) {
                    string = serde_json::to_string(&string).unwrap_or_else(|_| String::new());
                    window
                        .webview
                        .ExecuteScript(
                            PCWSTR(
                                utils::create_utf_string(format!("window.glorpClient.parseArgs({})", string)).as_ptr(),
                            ),
                            None,
                        )
                        .ok();
                }
            }

            _ => (),
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe extern "system" fn wnd_proc_subwindow(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let window_data_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Window;
        if window_data_ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let window = &mut *window_data_ptr;

        match msg {
            WM_SETFOCUS => {
                let child = GetWindow(hwnd, GW_CHILD).ok();
                if child.is_some() {
                    SetFocus(child).ok();
                }
            }
            WM_DESTROY => {
                window.controller.Close().ok();
                drop(Box::from_raw(window_data_ptr));
                let count = WINDOW_COUNT.fetch_sub(1, Ordering::SeqCst);

                if count == 1 {
                    PostQuitMessage(0);
                }
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
                if WINDOW_COUNT.load(Ordering::SeqCst) != 1 {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let window = create_main_window(Some(window.env.clone()));
                let cds_ptr = lparam.0 as *mut COPYDATASTRUCT;
                let cds = &*cds_ptr;
                let data = std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize);
                if let Ok(string) = String::from_utf8(data.to_vec()) {
                    window
                        .webview
                        .ExecuteScript(
                            PCWSTR(
                                utils::create_utf_string(format!("window.glorpClient.parseArgs('{}')", string))
                                    .as_ptr(),
                            ),
                            None,
                        )
                        .ok();
                }
            }

            _ => (),
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
