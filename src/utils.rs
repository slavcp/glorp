#![allow(non_snake_case)]

use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM},
    UI::WindowsAndMessaging::*,
};

pub fn create_utf_string(string: &str) -> Vec<u16> {
    let mut string_utf: Vec<u16> = string.encode_utf16().collect();
    string_utf.push(0);
    string_utf
}

pub fn LOWORD(l: usize) -> usize {
    l & 0xffff
}

pub fn HIWORD(l: usize) -> usize {
    (l >> 16) & 0xffff
}

pub fn find_child_window_by_class(parent: HWND, class_name: &str) -> HWND {
    let mut data = (HWND::default(), class_name);

    extern "system" fn enum_child_proc(handle: HWND, lparam: LPARAM) -> BOOL {
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
    unsafe {
        if let BOOL(1) = EnumChildWindows(
            Some(parent),
            Some(enum_child_proc),
            LPARAM(&mut data as *mut (HWND, &str) as _),
        ) {
            eprint!("Could not find child window")
        }

        data.0
    }
}
