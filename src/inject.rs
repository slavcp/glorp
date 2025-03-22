use crate::config::Config;
use windows::{
    Win32::{
        Foundation::*,
        System::{
            Diagnostics::Debug::*, Diagnostics::ToolHelp::*, LibraryLoader::*, Memory::*,
            Threading::*,
        },
    },
    core::*,
};
pub struct DllInjector {
    process_name: String,
    dll_path: String,
    renderer: bool,
}

impl DllInjector {
    pub fn new(process_name: &str, dll_path: &str, renderer: bool) -> Self {
        let injector = Self {
            process_name: process_name.to_string(),
            dll_path: dll_path.to_string(),
            renderer,
        };
        unsafe { injector.inject() };
        injector
    }

    pub unsafe fn inject(&self) {
        unsafe {
            if let Some(process_id) = self.get_proc_id() {
                let process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, process_id).unwrap();
                let load_library = GetProcAddress(
                    GetModuleHandleW(w!("kernel32.dll")).unwrap(),
                    s!("LoadLibraryW"),
                )
                .unwrap();

                let dll_path_bytes: Vec<u16> = self
                    .dll_path
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                let alloc = VirtualAllocEx(
                    process_handle,
                    None,
                    dll_path_bytes.len() * 2,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                );

                WriteProcessMemory(
                    process_handle,
                    alloc,
                    dll_path_bytes.as_ptr() as _,
                    dll_path_bytes.len() * 2,
                    None,
                )
                .unwrap();

                CreateRemoteThread(
                    process_handle,
                    None,
                    0,
                    Some(std::mem::transmute::<
                        unsafe extern "system" fn() -> isize,
                        unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
                    >(load_library)),
                    Some(alloc),
                    0,
                    None,
                )
                .unwrap();

                CloseHandle(process_handle).ok();
            }
        }
    }

    unsafe fn get_proc_id(&self) -> Option<u32> {
        unsafe {
            let mut process_entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..PROCESSENTRY32W::default()
            };
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).unwrap();

            if Process32FirstW(snapshot, &mut process_entry).is_ok() {
                let mut last_process_name = String::new();
                loop {
                    let found_process_name = String::from_utf16_lossy(&process_entry.szExeFile);

                    if found_process_name.contains(&self.process_name)
                        && ((self.renderer && is_renderer(process_entry.th32ProcessID))
                            || (!self.renderer && last_process_name.contains("glorp.exe")))
                    {
                        CloseHandle(snapshot).ok();
                        return Some(process_entry.th32ProcessID);
                    }

                    last_process_name = found_process_name;
                    if Process32NextW(snapshot, &mut process_entry).is_err() {
                        break;
                    }
                }
            }

            CloseHandle(snapshot).ok();
            None
        }
    }
}

// check if d3d11.dll is loaded
unsafe fn is_renderer(pid: u32) -> bool {
    unsafe {
        let mut module_entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..MODULEENTRY32W::default()
        };
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
            .unwrap_or_default();

        if Module32FirstW(snapshot, &mut module_entry).is_ok() {
            loop {
                let module_name = String::from_utf16_lossy(&module_entry.szModule);

                if module_name.contains("d3d11.dll") {
                    CloseHandle(snapshot).ok();
                    return true;
                }

                if Module32NextW(snapshot, &mut module_entry).is_err() {
                    break;
                }
            }
        }
        CloseHandle(snapshot).ok();
        false
    }
}
pub unsafe fn hook_webview2(config: &std::sync::Arc<std::sync::Mutex<Config>>) {
    let current_exe = std::env::current_exe().unwrap();
    DllInjector::new(
        "msedgewebview2.exe",
        current_exe
            .parent()
            .unwrap()
            .join("webview.dll")
            .to_str()
            .unwrap(),
        false,
    );
    if config.lock().unwrap().get("hardFlip") {
        DllInjector::new(
            "msedgewebview2.exe",
            current_exe
                .parent()
                .unwrap()
                .join("render.dll")
                .to_str()
                .unwrap(),
            true,
        );
    }
}
