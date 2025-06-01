use once_cell::sync::Lazy;
use std::mem::transmute;
use std::sync::atomic::{AtomicBool, AtomicPtr};
use std::sync::mpsc::{Sender, channel};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::Input::*;
use windows::Win32::{
    Foundation::BOOL,
    Foundation::*,
    System::{Diagnostics::Debug::*, SystemServices::*, Threading::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
use windows::core::*;

static SPACE_DOWN: INPUT = INPUT {
    r#type: INPUT_KEYBOARD,
    Anonymous: INPUT_0 {
        ki: KEYBDINPUT {
            wVk: VK_SPACE,
            wScan: 0,
            dwFlags: KEYBD_EVENT_FLAGS(0),
            time: 0,
            dwExtraInfo: 0,
        },
    },
};

static SPACE_UP: INPUT = INPUT {
    r#type: INPUT_KEYBOARD,
    Anonymous: INPUT_0 {
        ki: KEYBDINPUT {
            wVk: VK_SPACE,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        },
    },
};

static SCROLL_SENDER: Lazy<Sender<()>> = Lazy::new(|| {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        while let Ok(_) = rx.recv() {
            unsafe {
                SendInput(&[SPACE_DOWN], std::mem::size_of::<INPUT>() as i32);
                Sleep(5);
                SendInput(&[SPACE_UP], std::mem::size_of::<INPUT>() as i32);
            }
        }
    });
    tx
});

static mut PREV_WNDPROC_1: WNDPROC = None;
static mut PREV_WNDPROC_2: WNDPROC = None;

static LOCK_STATUS: AtomicBool = AtomicBool::new(false);
static WINDOW_HANDLE: AtomicPtr<HWND> = AtomicPtr::new(std::ptr::null_mut());

struct ChromeWindows {
    chrome_window: HWND,
    chrome_renderwidget: HWND,
}

impl ChromeWindows {
    fn get(parent: HWND) -> Self {
        ChromeWindows {
            chrome_window: Self::find_child_window_by_class(parent, "Chrome_WidgetWin_1"),
            chrome_renderwidget: Self::find_child_window_by_class(
                parent,
                "Chrome_RenderWidgetHostHWND",
            ),
        }
    }

    #[allow(clippy::fn_to_numeric_cast)]
    unsafe fn set_window_procs(&self) {
        unsafe {
            // set proc for chrome_window
            let original_proc_1 = GetWindowLongPtrW(self.chrome_window, GWLP_WNDPROC);
            PREV_WNDPROC_1 = transmute::<
                isize,
                Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>,
            >(original_proc_1);
            SetWindowLongPtrW(self.chrome_window, GWLP_WNDPROC, wnd_proc_1 as isize);

            // set proc for chrome_renderwidget
            let original_proc_2 = GetWindowLongPtrW(self.chrome_renderwidget, GWLP_WNDPROC);
            PREV_WNDPROC_2 = transmute::<
                isize,
                Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>,
            >(original_proc_2);
            SetWindowLongPtrW(
                self.chrome_renderwidget,
                GWLP_WNDPROC,
                wnd_proc_widget as isize,
            );
        }
    }

    fn find_child_window_by_class(parent: HWND, class_name: &str) -> HWND {
        unsafe {
            let mut data = (HWND::default(), class_name);

            if let BOOL(1) = EnumChildWindows(
                Some(parent),
                Some(find_child_window),
                LPARAM(&mut data as *mut (HWND, &str) as _),
            ) {
                OutputDebugStringW(w!("EnumChildWindowsFailed\0"));
            }

            data.0
        }
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(_: HINSTANCE, call_reason: u32, _: *mut ()) {
    if call_reason == DLL_PROCESS_ATTACH {
        attach();
    }
}

fn attach() {
    unsafe {
        let parent = FindWindowW(w!("krunker_webview"), PCWSTR::null()).unwrap();
        let handle_ptr = Box::into_raw(Box::new(parent)); // store on the heap so it stays alive
        WINDOW_HANDLE.store(handle_ptr, std::sync::atomic::Ordering::Relaxed);
        let chrome_windows = ChromeWindows::get(parent);
        chrome_windows.set_window_procs();

        std::thread::spawn(|| {
            let mut msg: MSG = MSG::default();
            // check whenever a window is created if it has the attribute Chrome.WindowTranslucent (the one that warns about pointer lock) and if it does, destroy it
            SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_CREATE,
                None,
                Some(window_event_proc),
                GetCurrentProcessId(),
                0,
                WINEVENT_OUTOFCONTEXT,
            );

            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    }
}

extern "system" fn find_child_window(handle: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let data = lparam.0 as *mut (HWND, &str);
        let target_class = (*data).1;

        let mut class_name: [u16; 256] = [0; 256];
        GetClassNameW(handle, &mut class_name);

        let window_class = String::from_utf16_lossy(&class_name);

        if window_class.contains(target_class) {
            (*data).0 = handle;
            return BOOL(0);
        }

        BOOL(1)
    }
}

#[unsafe(no_mangle)]
unsafe extern "system" fn wnd_proc_1(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            // when you press esc chromium puts a few seconds of delay before the pointer can get locked again as a security measure
            WM_CHAR => LRESULT(1),
            WM_KEYDOWN | WM_KEYUP => {
                if wparam.0 == VK_ESCAPE.0 as usize
                    && LOCK_STATUS.load(std::sync::atomic::Ordering::Relaxed)
                {
                    // glorp.exe (not the webview)
                    let glorp = WINDOW_HANDLE.load(std::sync::atomic::Ordering::Relaxed);
                    SetFocus(Some(*glorp)).ok();
                }
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                if LOCK_STATUS.load(std::sync::atomic::Ordering::Relaxed) {
                    return CallWindowProcW(
                        PREV_WNDPROC_1,
                        window,
                        WM_KEYDOWN,
                        WPARAM(VK_F20.0 as usize),
                        lparam,
                    );
                }
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            WM_LBUTTONUP => {
                CallWindowProcW(
                    PREV_WNDPROC_1,
                    window,
                    WM_KEYUP,
                    WPARAM(VK_F20.0 as usize),
                    lparam,
                );
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            WM_MOUSEMOVE | WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
                if LOCK_STATUS.load(std::sync::atomic::Ordering::Relaxed) {
                    return CallWindowProcW(
                        PREV_WNDPROC_1,
                        window,
                        message,
                        WPARAM(wparam.0 & !MK_LBUTTON.0 as usize),
                        lparam,
                    );
                }
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            WM_INPUT => {
                let raw_input_handle = HRAWINPUT(lparam.0 as _);
                let mut buffer: [u8; 48] = [0; 48];
                let mut size = 48;

                GetRawInputData(
                    raw_input_handle,
                    RID_INPUT,
                    Some(buffer.as_mut_ptr() as _),
                    &mut size,
                    std::mem::size_of::<RAWINPUTHEADER>() as u32,
                );

                let raw_input = buffer.as_mut_ptr() as *mut RAWINPUT;

                if (*raw_input).data.mouse.Anonymous.Anonymous.usButtonFlags != 0 {
                    return LRESULT(1);
                }
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            _ => CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam),
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "system" fn wnd_proc_widget(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_APP => {
                SetWindowLongPtrW(window, GWLP_WNDPROC, wnd_proc_widget_rampboost as isize);
                LRESULT(1)
            }
            WM_USER => {
                LOCK_STATUS.store(wparam.0 != 0, std::sync::atomic::Ordering::Relaxed);
                LRESULT(1)
            }
            WM_MOUSEWHEEL => {
                if LOCK_STATUS.load(std::sync::atomic::Ordering::Relaxed) {
                    let glorp = WINDOW_HANDLE.load(std::sync::atomic::Ordering::Relaxed);
                    // send the message to the glorp window, from where it gets sent as a js event, best fix i could find for the fps dropping when scrolling whilst still keeping scroll behaviour intact
                    PostMessageW(Some(*glorp), message, wparam, lparam).ok();
                    return LRESULT(1);
                }
                CallWindowProcW(PREV_WNDPROC_2, window, message, wparam, lparam)
            }
            _ => CallWindowProcW(PREV_WNDPROC_2, window, message, wparam, lparam),
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "system" fn wnd_proc_widget_rampboost(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_MOUSEWHEEL => {
                if LOCK_STATUS.load(std::sync::atomic::Ordering::Relaxed) {
                    SCROLL_SENDER.send(()).ok();
                    return LRESULT(1);
                }
                CallWindowProcW(PREV_WNDPROC_2, window, message, wparam, lparam)
            }
            WM_USER => {
                LOCK_STATUS.store(wparam.0 != 0, std::sync::atomic::Ordering::Relaxed);
                LRESULT(1)
            }
            _ => CallWindowProcW(PREV_WNDPROC_2, window, message, wparam, lparam),
        }
    }
}

unsafe extern "system" fn window_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    unsafe {
        let prop = GetPropW(hwnd, w!("Chrome.WindowTranslucent"));
        if !prop.is_invalid() {
            PostMessageW(Some(hwnd), WM_DESTROY, WPARAM(0), LPARAM(0)).ok();
        }
    }
}
