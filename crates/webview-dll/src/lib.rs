use std::{
    ffi::c_void,
    mem::{self, transmute},
    ptr,
    sync::{
        self, LazyLock,
        atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicUsize},
        mpsc::{Sender, channel},
    },
    thread,
};
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, LibraryLoader::GetModuleHandleW, SystemServices::*, Threading::*},
    UI::{
        Accessibility::*,
        Input::{KeyboardAndMouse::*, *},
        WindowsAndMessaging::*,
    },
};
use windows::core::*;

fn debug_print(message: impl AsRef<str>) {
    let mut wide: Vec<u16> = message.as_ref().encode_utf16().collect();
    wide.push(0);
    unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())) };
}

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

static SCROLL_SENDER: LazyLock<Sender<()>> = LazyLock::new(|| {
    let (tx, rx) = channel();
    thread::spawn(move || {
        debug_print(format!("webview: rampboost input thread started id={}", unsafe { GetCurrentThreadId() }));
        while rx.recv().is_ok() {
            unsafe {
                let down = SendInput(&[SPACE_DOWN], mem::size_of::<INPUT>() as i32);
                Sleep(5);
                let up = SendInput(&[SPACE_UP], mem::size_of::<INPUT>() as i32);
                if down != 1 || up != 1 {
                    debug_print(format!("webview: rampboost SendInput incomplete down={down} up={up}"));
                }
            }
        }
        debug_print("webview: rampboost input thread ended");
    });
    tx
});

static mut PREV_WNDPROC_1: WNDPROC = None;
static mut PREV_WNDPROC_2: WNDPROC = None;

static DRAG_STATUS: AtomicBool = AtomicBool::new(false);
static WINDOW_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HOOK_HANDLE: AtomicUsize = AtomicUsize::new(0);

struct ChromeWindows {
    chrome_window: HWND,
    chrome_renderwidget: HWND,
}

unsafe extern "system" fn dummy_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn spawn_injected_audio_window() {
    let hinstance = unsafe { GetModuleHandleW(None).unwrap().into() };

    let class_name = w!("Audio_Target_Class");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(dummy_wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED,
            class_name,
            w!("audio window"),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            100,
            100,
            None,
            None,
            Some(hinstance),
            None,
        )
        .unwrap()
    };

    unsafe {
        // makes window transparent
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);

        // set an invisible owner window
        // drops it from the taskbar
        let desktop_hwnd = GetDesktopWindow();
        SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, desktop_hwnd.0 as isize);
    }

    std::thread::park()
}

impl ChromeWindows {
    fn get(parent: HWND) -> Self {
        let windows = ChromeWindows {
            chrome_window: Self::find_child_window_by_class(parent, "Chrome_WidgetWin_1"),
            chrome_renderwidget: Self::find_child_window_by_class(parent, "Chrome_RenderWidgetHostHWND"),
        };
        debug_print(format!(
            "webview: child windows parent={:?} chrome={:?} render_widget={:?}",
            parent, windows.chrome_window, windows.chrome_renderwidget
        ));
        windows
    }

    unsafe fn set_window_procs(&self) {
        unsafe {
            if self.chrome_window.0.is_null() || self.chrome_renderwidget.0.is_null() {
                debug_print("webview: cannot install window procedures because a Chrome child window is missing");
                return;
            }
            // set proc for chrome_window
            let original_proc_1 = GetWindowLongPtrW(self.chrome_window, GWLP_WNDPROC);
            debug_print(format!("webview: original chrome wndproc={original_proc_1:#x}"));
            PREV_WNDPROC_1 = transmute::<isize, Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>>(original_proc_1);
            let previous = SetWindowLongPtrW(self.chrome_window, GWLP_WNDPROC, wnd_proc_1 as *const () as isize);
            debug_print(format!("webview: installed chrome wndproc, previous={previous:#x}"));

            // set proc for chrome_renderwidget
            let original_proc_2 = GetWindowLongPtrW(self.chrome_renderwidget, GWLP_WNDPROC);
            debug_print(format!("webview: original render widget wndproc={original_proc_2:#x}"));
            PREV_WNDPROC_2 = transmute::<isize, Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>>(original_proc_2);
            let previous = SetWindowLongPtrW(self.chrome_renderwidget, GWLP_WNDPROC, wnd_proc_widget as *const () as isize);
            debug_print(format!("webview: installed render widget wndproc, previous={previous:#x}"));
        }
    }

    fn find_child_window_by_class(parent: HWND, class_name: &str) -> HWND {
        unsafe {
            let mut data = (HWND::default(), class_name);

            if let BOOL(1) = EnumChildWindows(Some(parent), Some(find_child_window), LPARAM(&mut data as *mut (HWND, &str) as _)) {
                debug_print(format!("webview: EnumChildWindows failed parent={:?} class={class_name}", parent));
            }

            data.0
        }
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(_: HINSTANCE, call_reason: u32, _: *mut ()) {
    match call_reason {
        DLL_PROCESS_ATTACH => {
            debug_print("webview: DLL_PROCESS_ATTACH");
            attach()
        }
        DLL_PROCESS_DETACH => {
            debug_print("webview: DLL_PROCESS_DETACH");
            detach()
        }
        _ => (),
    }
}

static THREAD_ID: AtomicU32 = AtomicU32::new(0);

fn detach() {
    debug_print("webview: detaching hooks and message thread");
    unsafe {
        let hook_raw = HOOK_HANDLE.load(sync::atomic::Ordering::Relaxed);
        if hook_raw != 0 {
            let result = UnhookWinEvent(HWINEVENTHOOK(hook_raw as _));
            debug_print(format!("webview: UnhookWinEvent result={result:?}"));
        }

        //  terminate the message loop otherwise launching just crashes if webview2 is still running
        let thread_id = THREAD_ID.load(sync::atomic::Ordering::Relaxed);
        if thread_id != 0 {
            let result = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            debug_print(format!("webview: posted message-thread shutdown result={result:?}"));
        }
    }
}

fn attach() {
    debug_print("webview: attach started");
    unsafe {
        let parent = match FindWindowW(w!("krunker_webview"), PCWSTR::null()) {
            Ok(parent) => parent,
            Err(error) => {
                debug_print(format!("webview: main window not found: {error}"));
                return;
            }
        };
        debug_print(format!("webview: found main window={parent:?}"));
        WINDOW_HANDLE.store(parent.0, sync::atomic::Ordering::Relaxed);
        let chrome_windows = ChromeWindows::get(parent);
        chrome_windows.set_window_procs();

        // thread to check if the parent window has disappeared
        thread::spawn(move || {
            debug_print("webview: parent-window monitor thread started");
            loop {
                let current_parent = HWND(WINDOW_HANDLE.load(sync::atomic::Ordering::Relaxed));

                if !IsWindow(Some(current_parent)).as_bool() {
                    let new_parent = FindWindowW(w!("krunker_webview"), PCWSTR::null());

                    if let Ok(new_parent) = new_parent {
                        WINDOW_HANDLE.store(new_parent.0, sync::atomic::Ordering::Relaxed);
                        debug_print(format!("webview: main window recreated={new_parent:?}"));

                        let new_chrome_windows = ChromeWindows::get(new_parent);
                        new_chrome_windows.set_window_procs();
                    }
                }

                Sleep(5000);
            }
        });

        thread::spawn(move || {
            spawn_injected_audio_window();
        });

        thread::spawn(move || {
            THREAD_ID.store(GetCurrentThreadId(), sync::atomic::Ordering::Relaxed);
            debug_print(format!("webview: WinEvent message thread started id={}", GetCurrentThreadId()));
            let mut msg: MSG = MSG::default();
            // check whenever a window is created if it has the attribute Chrome.WindowTranslucent (the one that warns about pointer lock) and if it does, destroy it
            let hook = SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_CREATE,
                None,
                Some(window_event_proc),
                GetCurrentProcessId(),
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            HOOK_HANDLE.store(hook.0 as usize, sync::atomic::Ordering::Relaxed);
            debug_print(format!("webview: SetWinEventHook handle={:?}", hook));

            loop {
                if GetMessageW(&mut msg, None, 0, 0).into() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    debug_print("webview: message loop ended");
                    break;
                }
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

        // let window_class = String::from_utf16_lossy(&class_name);

        // if window_class.contains(target_class) {
        //     (*data).0 = handle;
        //     return BOOL(0);
        // }

        // no heap alloc for this. cuts a bit of memory
        // even if there isnt much to begin with on glorps executable lol
        let len = class_name.iter().position(|&c| c == 0).unwrap_or(256);
        let class_slice = &class_name[..len];
        let mut target_wide = [0u16; 64];
        let mut target_len = 0;
        for c in target_class.encode_utf16() {
            target_wide[target_len] = c;
            target_len += 1;
        }
        let target_slice = &target_wide[..target_len];
        if class_slice.windows(target_len).any(|w| w == target_slice) {
            (*data).0 = handle;
            return BOOL(0);
        }

        BOOL(1)
    }
}

#[unsafe(no_mangle)]
unsafe extern "system" fn wnd_proc_1(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_CHAR => LRESULT(1),
            WM_QUIT => {
                debug_print("webview: chrome wndproc received WM_QUIT");
                detach();
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            // when you press esc chromium puts a few seconds of delay before the pointer can get locked again as a security measure
            WM_KEYDOWN | WM_KEYUP => {
                if wparam.0 == VK_ESCAPE.0 as usize && DRAG_STATUS.load(sync::atomic::Ordering::Relaxed) {
                    // glorp.exe (not the webview)
                    let glorp = WINDOW_HANDLE.load(sync::atomic::Ordering::Relaxed);
                    let result = SetFocus(Some(HWND(glorp)));
                    debug_print(format!("webview: redirected Escape focus to client result={result:?}"));
                }
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN | WM_RBUTTONDBLCLK | WM_XBUTTONDOWN | WM_NCXBUTTONDBLCLK | WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                CallWindowProcW(PREV_WNDPROC_1, window, message, WPARAM(wparam.0 & !MK_LBUTTON.0 as usize), lparam)
            }
            WM_MOUSEMOVE => {
                if DRAG_STATUS.load(sync::atomic::Ordering::Relaxed) {
                    return CallWindowProcW(PREV_WNDPROC_1, window, message, WPARAM(wparam.0 & !MK_LBUTTON.0 as usize), lparam);
                }
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            WM_INPUT => {
                let mut buffer = std::mem::MaybeUninit::<RAWINPUT>::uninit();
                let mut size = std::mem::size_of::<RAWINPUT>() as u32;
                /*
                2 syscalls to GetRawInputData per call is NOT what we want
                we don't care about anything else other than denying any events that have a MB press, raw input behaviour itself is handled wv2

                */

                if GetRawInputData(
                    HRAWINPUT(lparam.0 as _),
                    RID_INPUT,
                    Some(buffer.as_mut_ptr() as _),
                    &mut size,
                    mem::size_of::<RAWINPUTHEADER>() as u32,
                ) != u32::MAX
                {
                    let raw = buffer.assume_init_ref();

                    if raw.data.mouse.Anonymous.Anonymous.usButtonFlags != 0 {
                        return LRESULT(1);
                    };
                }
                CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam)
            }
            _ => CallWindowProcW(PREV_WNDPROC_1, window, message, wparam, lparam),
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "system" fn wnd_proc_widget(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_USER => {
                // 1 = change proc to wnd_proc_widget_rampboost
                // 2 or 0 = allow-drag status
                if wparam.0 == 1 {
                    debug_print("webview: enabling rampboost window procedure");
                    SetWindowLongPtrW(window, GWLP_WNDPROC, wnd_proc_widget_rampboost as *const () as isize);
                } else {
                    DRAG_STATUS.store(wparam.0 == 2, sync::atomic::Ordering::Relaxed);
                    debug_print(format!("webview: drag status={}", wparam.0 == 2));
                }
                LRESULT(1)
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL | WM_POINTERWHEEL | WM_POINTERHWHEEL => {
                if DRAG_STATUS.load(sync::atomic::Ordering::Relaxed) {
                    let glorp = WINDOW_HANDLE.load(sync::atomic::Ordering::Relaxed);
                    // send the message to the glorp window, from where it gets sent as a js event
                    // best fix i could find for the fps dropping when scrolling whilst still keeping scroll behaviour intact
                    PostMessageW(Some(HWND(glorp)), message, wparam, lparam).ok();
                    return LRESULT(1);
                }
                CallWindowProcW(PREV_WNDPROC_2, window, message, wparam, lparam)
            }
            _ => CallWindowProcW(PREV_WNDPROC_2, window, message, wparam, lparam),
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "system" fn wnd_proc_widget_rampboost(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL | WM_POINTERWHEEL | WM_POINTERHWHEEL => {
                if DRAG_STATUS.load(sync::atomic::Ordering::Relaxed) {
                    SCROLL_SENDER.send(()).ok();
                    return LRESULT(1);
                }
                CallWindowProcW(PREV_WNDPROC_2, window, message, wparam, lparam)
            }
            WM_USER => {
                // 3 = change proc to wnd_proc_widget
                // 2 or 0 = allow-drag status
                if wparam.0 == 3 {
                    debug_print("webview: disabling rampboost window procedure");
                    SetWindowLongPtrW(window, GWLP_WNDPROC, wnd_proc_widget as *const () as isize);
                } else {
                    DRAG_STATUS.store(wparam.0 == 2, sync::atomic::Ordering::Relaxed);
                    debug_print(format!("webview: rampboost drag status={}", wparam.0 == 2));
                }
                LRESULT(1)
            }
            _ => CallWindowProcW(PREV_WNDPROC_2, window, message, wparam, lparam),
        }
    }
}

unsafe extern "system" fn window_event_proc(_hook: HWINEVENTHOOK, _event: u32, hwnd: HWND, _id_object: i32, _id_child: i32, _thread: u32, _time: u32) {
    unsafe {
        let prop = GetPropW(hwnd, w!("Chrome.WindowTranslucent"));
        if !prop.is_invalid() {
            debug_print(format!("webview: destroying translucent Chrome window={hwnd:?}"));
            PostMessageW(Some(hwnd), WM_DESTROY, WPARAM(0), LPARAM(0)).ok();
        }
    }
}
