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
    dll_path: String,
    pid: u32,
}

static ERROR_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

impl DllInjector {
    pub fn new(dll_path: &str, pid: u32) -> Self {
        Self {
            dll_path: dll_path.to_string(),
            pid,
        }
    }

    fn handle_error(&mut self, error_msg: &str) {
        println!("{}", error_msg);
        let error = PCWSTR(error_msg.encode_utf16().collect::<Vec<u16>>().as_ptr());
        unsafe {
            OutputDebugStringW(error);
        }

        let current = ERROR_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if current >= 1 {
            unsafe {
                MessageBoxW(
                    None,
                    w!("Error injecting dlls, please retry launching"),
                    error,
                    MB_ICONERROR | MB_SYSTEMMODAL,
                );
                super::utils::kill("msedgewebview2.exe");
                std::process::exit(0);
            }
        };
        // retry
        std::thread::sleep(std::time::Duration::from_secs(1));
        self.inject();
    }

    pub fn inject(&mut self) {
        unsafe {
            let process_handle = match OpenProcess(PROCESS_ALL_ACCESS, false, self.pid) {
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

            if is_dll_loaded(self.pid, "webview.dll") {
                // unloading the existing DLL
                let mut module_entry = MODULEENTRY32W {
                    dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
                    ..MODULEENTRY32W::default()
                };

                let module_snapshot = match CreateToolhelp32Snapshot(
                    TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                    self.pid,
                ) {
                    Ok(handle) => handle,
                    Err(_) => return,
                };

                if Module32FirstW(module_snapshot, &mut module_entry).is_ok() {
                    loop {
                        let module_name = String::from_utf16_lossy(&module_entry.szModule);
                        if module_name.contains("webview.dll") {
                            if let Some(free_library) = GetProcAddress(kernel32, s!("FreeLibrary"))
                            {
                                CreateRemoteThread(
                                    process_handle,
                                    None,
                                    0,
                                    Some(std::mem::transmute::<
                                        unsafe extern "system" fn() -> isize,
                                        unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
                                    >(free_library)),
                                    Some(module_entry.modBaseAddr as *mut std::ffi::c_void),
                                    0,
                                    None,
                                )
                                .ok();
                            }
                            break;
                        }

                        if Module32NextW(module_snapshot, &mut module_entry).is_err() {
                            break;
                        }
                    }
                }
                CloseHandle(module_snapshot).ok();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }

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
        }
    }
}

fn is_dll_loaded(pid: u32, dll_name: &str) -> bool {
    unsafe {
        let mut module_entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..MODULEENTRY32W::default()
        };

        let module_snapshot =
            match CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) {
                Ok(handle) => handle,
                Err(_) => {
                    return false;
                }
            };

        if Module32FirstW(module_snapshot, &mut module_entry).is_ok() {
            loop {
                let module_name = String::from_utf16_lossy(&module_entry.szModule);
                if module_name.contains(dll_name) {
                    CloseHandle(module_snapshot).ok();
                    return true;
                }

                if Module32NextW(module_snapshot, &mut module_entry).is_err() {
                    break;
                }
            }
        }

        CloseHandle(module_snapshot).ok();
        false
    }
}

fn find_renderer_process(parent_pid: u32) -> Result<u32> {
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
        if Process32FirstW(snapshot, &mut process_entry).is_ok() {
            loop {
                let process_name = String::from_utf16_lossy(&process_entry.szExeFile);
                let is_webview = process_name.to_lowercase().contains("msedgewebview2.exe");
                let is_child = process_entry.th32ParentProcessID == parent_pid;

                if is_webview && is_child && is_dll_loaded(process_entry.th32ProcessID, "d3d11.dll")
                {
                    CloseHandle(snapshot).ok();
                    return Ok(process_entry.th32ProcessID);
                }

                if Process32NextW(snapshot, &mut process_entry).is_err() {
                    break;
                }
            }
        }

        CloseHandle(snapshot).ok();
        Err(windows::core::Error::new(
            windows::core::HRESULT(-1),
            "Renderer process not found",
        ))
    }
}
pub fn hook_webview2(hard_flip: bool, pid: u32) {
    let current_exe = std::env::current_exe().unwrap();
    let mut webview_injector = DllInjector::new(
        current_exe
            .parent()
            .unwrap()
            .join("webview.dll")
            .to_str()
            .unwrap(),
        pid,
    );
    webview_injector.inject();
    if hard_flip {
        if let Ok(renderer_pid) = find_renderer_process(pid) {
            let mut render_injector = DllInjector::new(
                current_exe
                    .parent()
                    .unwrap()
                    .join("render.dll")
                    .to_str()
                    .unwrap(),
                renderer_pid,
            );
            render_injector.inject();
        } else {
            eprintln!("Failed to find renderer process");
        }
    }
}
