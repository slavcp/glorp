//! Consumer side of the OBS shared-texture capture — discovering the producer and driving the
//! shared control block.
//!
//! The producer is the **WebView2 GPU process** running `render.dll` (as `vk_swiftshader.dll`).
//! Because only that process ever publishes `GlorpCaptureInfo_<pid>`, scanning every
//! `msedgewebview2.exe` and validating magic/version disambiguates the GPU process out of the
//! crowd — and naturally re-discovers it after a GPU-process crash/respawn under a new PID.

#![allow(non_camel_case_types, dead_code)]

use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::{
            Diagnostics::ToolHelp::*,
            Memory::*,
            Threading::*,
        },
    },
};

pub const GLORP_CAPTURE_MAGIC: u32 = 0x5052_4347; // "GCRP"
pub const GLORP_CAPTURE_VERSION: u32 = 1;
pub const READER_ACTIVE: u32 = 0x1;
const INFO_SIZE: usize = 64;
/// SYNCHRONIZE access right needed to peek (wait) on the frame event.
const SYNCHRONIZE: u32 = 0x0010_0000;

/// Layout must match the producer's `GlorpCaptureInfo` byte-for-byte (docs/obs-shared-capture.md §4).
#[repr(C, packed)]
pub struct GlorpCaptureInfo {
    pub magic: u32,
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub flags: u32,
    pub frame_counter: u64,
    pub reserved: [u64; 2],
}

/// An open handle to one producer's capture state.
pub struct Session {
    pub pid: u32,
    pub mapping: HANDLE,
    pub frame_event: HANDLE,
    pub view: MEMORY_MAPPED_VIEW_ADDRESS,
    pub info: *mut GlorpCaptureInfo,
}

impl Session {
    /// Close handles + unmap. The producer's named objects vanish when their last handle closes.
    pub fn close(&mut self) {
        unsafe {
            if !self.view.Value.is_null() {
                let _ = UnmapViewOfFile(self.view);
            }
            if !self.mapping.0.is_null() {
                let _ = CloseHandle(self.mapping);
            }
            if !self.frame_event.0.is_null() {
                let _ = CloseHandle(self.frame_event);
            }
        }
    }
}

pub fn set_reader_active(s: &Session, on: bool) {
    unsafe {
        if !s.info.is_null() {
            let info = &mut *s.info;
            if on {
                info.flags |= READER_ACTIVE;
            } else {
                info.flags &= !READER_ACTIVE;
            }
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn name_matches(name: &[u16], expected: &str) -> bool {
    let exp: Vec<u16> = expected.encode_utf16().collect();
    if name.len() <= exp.len() {
        return false;
    }
    name[..exp.len()] == exp[..] && name[exp.len()] == 0
}

/// Try to open + validate the producer for `pid`. Returns a session on success (mapping mapped +
/// magic/version valid), otherwise `None` and nothing leaks.
fn try_attach(pid: u32) -> Option<Session> {
    unsafe {
        let info_name = wide(&format!("GlorpCaptureInfo_{pid}"));
        let mapping =
            OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, PCWSTR(info_name.as_ptr())).ok()?;
        let view = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, INFO_SIZE);
        if view.Value.is_null() {
            let _ = CloseHandle(mapping);
            return None;
        }
        let info = view.Value as *mut GlorpCaptureInfo;
        if (*info).magic != GLORP_CAPTURE_MAGIC || (*info).version != GLORP_CAPTURE_VERSION {
            let _ = UnmapViewOfFile(view);
            let _ = CloseHandle(mapping);
            return None;
        }

        super::debug_print(format!("capture: producer discovered (pid {pid})"));

        // Frame event is optional for correctness (we poll frame_counter), so a failure here is
        // non-fatal — pass a null/invalid handle.
        let event_name = wide(&format!("GlorpCaptureFrame_{pid}"));
        let frame_event =
            OpenEventW(SYNCHRONIZATION_ACCESS_RIGHTS(SYNCHRONIZE), false, PCWSTR(event_name.as_ptr()))
                .unwrap_or(HANDLE::default());

        Some(Session {
            pid,
            mapping,
            frame_event,
            view,
            info,
        })
    }
}

/// True if a process with `pid` currently exists (does not wait, needs no rights).
pub fn process_exists(pid: u32) -> bool {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_ACCESS_RIGHTS(0), false, pid) else {
            return false;
        };
        let exists = !h.0.is_null();
        let _ = CloseHandle(h);
        exists
    }
}

/// Enumerate every `msedgewebview2.exe` and (re)attach to the one that publishes a valid capture
/// control block.
pub fn discover() -> Option<Session> {
    unsafe {
        let snapshot =
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut pe: PROCESSENTRY32W = std::mem::zeroed();
        pe.dwSize = size_of::<PROCESSENTRY32W>() as u32;

        let mut ok = Process32FirstW(snapshot, &mut pe);
        while ok.is_ok() {
            if name_matches(&pe.szExeFile, "msedgewebview2.exe") {
                if let Some(s) = try_attach(pe.th32ProcessID) {
                    let _ = CloseHandle(snapshot);
                    return Some(s);
                }
            }
            // dwSize must be reset before each next call.
            pe.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            ok = Process32NextW(snapshot, &mut pe);
        }
        let _ = CloseHandle(snapshot);
    }
    super::debug_print("capture: producer not found");
    None
}
