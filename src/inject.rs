use crate::config::Config;
use windows::{
    Win32::{
        Foundation::*,
        System::{
            Diagnostics::Debug::*, Diagnostics::ToolHelp::*, LibraryLoader::*, Memory::*,
            Threading::*,
        },
        UI::WindowsAndMessaging::*,
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
        let mut injector = Self {
            process_name: process_name.to_string(),
            dll_path: dll_path.to_string(),
            renderer,
        };
        injector.inject();
        injector
    }

    fn handle_error(&mut self, error_msg: &str) {
        unsafe {
            OutputDebugStringW(PCWSTR(
                error_msg.encode_utf16().collect::<Vec<u16>>().as_ptr(),
            ));
        }

        std::process::exit(0);
    }

    pub fn inject(&mut self) {
        unsafe {
            if let Ok(process_id) = self.get_proc_id() {
                let process_handle = match OpenProcess(PROCESS_ALL_ACCESS, false, process_id) {
                    Ok(handle) => handle,
                    Err(e) => {
                        self.handle_error(&format!("Failed to open process: {}", e));
                        return;
                    }
                };

                let kernel32 = match GetModuleHandleW(w!("kernel32.dll")) {
                    Ok(handle) => handle,
                    Err(e) => {
                        self.handle_error(&format!("Failed to get kernel32 handle: {}", e));
                        return;
                    }
                };

                let load_library = match GetProcAddress(kernel32, s!("LoadLibraryW")) {
                    Some(addr) => addr,
                    None => {
                        self.handle_error("Failed to get LoadLibraryW address");
                        return;
                    }
                };

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

                if let Err(e) = WriteProcessMemory(
                    process_handle,
                    alloc,
                    dll_path_bytes.as_ptr() as _,
                    dll_path_bytes.len() * 2,
                    None,
                ) {
                    self.handle_error(&format!("Failed to write to process memory: {}", e));
                    return;
                }

                if let Err(e) = CreateRemoteThread(
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
                ) {
                    self.handle_error(&format!("Failed to create remote thread: {}", e));
                    return;
                }

                CloseHandle(process_handle).ok();
            } else {
                self.handle_error("Failed to find target process");
            }
        }
    }

    fn get_proc_id(&self) -> Result<u32> {
        unsafe {
            let mut process_entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(handle) => handle,
                Err(e) => {
                    let msg = format!("Failed to create snapshot: {}", e);
                    OutputDebugStringW(PCWSTR(
                        msg.encode_utf16()
                            .chain(std::iter::once(0))
                            .collect::<Vec<u16>>()
                            .as_ptr(),
                    ));
                    return Err(e);
                }
            };

            // find parent process (glorp.exe)
            let mut parent_pid = None;
            if Process32FirstW(snapshot, &mut process_entry).is_ok() {
                loop {
                    let process_name = String::from_utf16_lossy(&process_entry.szExeFile);
                    if process_name.to_lowercase().contains("glorp.exe") {
                        parent_pid = Some(process_entry.th32ProcessID);
                        break;
                    }
                    if Process32NextW(snapshot, &mut process_entry).is_err() {
                        break;
                    }
                }
            }

            if parent_pid.is_none() {
                let msg = "Parent process (glorp.exe) not found";
                OutputDebugStringW(PCWSTR(
                    msg.encode_utf16()
                        .chain(std::iter::once(0))
                        .collect::<Vec<u16>>()
                        .as_ptr(),
                ));
            }

            if let Some(parent_pid) = parent_pid {
                Process32FirstW(snapshot, &mut process_entry).ok();
                loop {
                    let process_name = String::from_utf16_lossy(&process_entry.szExeFile);
                    let is_target = process_name.to_lowercase().contains(&self.process_name);
                    let is_child = process_entry.th32ParentProcessID == parent_pid;
                    let is_renderer = self.renderer && is_renderer(process_entry.th32ProcessID);

                    if is_target && (is_renderer || (!self.renderer && is_child)) {
                        CloseHandle(snapshot).ok();
                        return Ok(process_entry.th32ProcessID);
                    }

                    if Process32NextW(snapshot, &mut process_entry).is_err() {
                        break;
                    }
                }
            }

            CloseHandle(snapshot).ok();
            MessageBoxW(
                None,
                w!(
                    "IF YOU JUST UPDATED IGNORE THE MESSAGE AND LAUNCH THE CLIENT AGAIN\n
                    Error injecting dlls. this is usually because the WebView2 runtime is not running at the same process level as glorp.exe; if you ran the client as admin, restart it as normal and see if it works."
                ),
                w!("Error"),
                MB_ICONERROR | MB_YESNO | MB_SYSTEMMODAL,
            );
            Err(windows::core::Error::new::<&str>(
                windows::core::HRESULT(-1),
                "Process not found",
            ))
        }
    }
}

// check if d3d11.dll is loaded
fn is_renderer(pid: u32) -> bool {
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
pub fn hook_webview2(config: &std::sync::Arc<std::sync::Mutex<Config>>) {
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
