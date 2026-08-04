use std::{
    ffi::c_void,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{
            Direct3D11::*,
            Dxgi::{Common::*, *},
        },
        System::{Memory::*, Threading::*},
    },
    core::*,
};

/// "GCRP" little-endian — validates the control block in the consumer.
const MAGIC: u32 = 0x5052_4347;
const VERSION: u32 = 1;
/// Control block layout — see docs/obs-shared-capture.md §4.
/// MUST remain byte-for-byte compatible with what the consumer reads.
#[repr(C, packed)]
struct GlorpCaptureInfo {
    magic: u32,
    version: u32,
    width: u32,
    height: u32,
    format: u32,
    flags: u32,
    frame_counter: u64,
    reserved: [u64; 2],
}

/// Raw pointer to the mapped control block, kept in an atomic so the hot present path can read
/// `READER_ACTIVE` without taking the (uncontended) state mutex. 0 = capture not initialized.
static INFO_PTR: AtomicU64 = AtomicU64::new(0);

struct Capture {
    pid: u32,
    mapping: usize,
    frame_event: usize,
    /// Open NT handle (as `usize`) that keeps the `GlorpCaptureTex_<pid>` name resolvable. Must
    /// stay open for as long as the texture is published (a name only resolves while alive).
    shared_handle: Option<usize>,
    /// The real D3D11 device/context from the WebView2 composition swap chain.
    device: Option<ID3D11Device>,
    context: Option<ID3D11DeviceContext>,
    shared_tex: Option<ID3D11Texture2D>,
    cached_w: u32,
    cached_h: u32,
    cached_format: u32,
}

impl Drop for Capture {
    fn drop(&mut self) {
        crate::debug_print(format!("capture: dropping state for pid {}", self.pid));
        unsafe {
            let mapping = HANDLE(self.mapping as *mut _);
            if !mapping.0.is_null() {
                let _ = CloseHandle(mapping);
            }
            let frame_event = HANDLE(self.frame_event as *mut _);
            if !frame_event.0.is_null() {
                let _ = CloseHandle(frame_event);
            }
            if let Some(h) = self.shared_handle {
                let _ = CloseHandle(HANDLE(h as *mut _));
            }
        }
        // COM fields (device/context/shared_tex) are dropped here, releasing their references.
    }
}

impl Capture {
    /// Close the published name-handle and release the shared texture so it is re-created lazily
    /// (first use, or on the current swap chain / dims).
    fn release_shared(&mut self) {
        if self.shared_handle.is_some() || self.shared_tex.is_some() {
            crate::debug_print("capture: releasing published shared texture");
        }
        if let Some(h) = self.shared_handle.take() {
            unsafe {
                let _ = CloseHandle(HANDLE(h as *mut _));
            }
        }
        self.shared_tex = None;
    }
}

static CAPTURE: LazyLock<Mutex<Option<Capture>>> = LazyLock::new(|| Mutex::new(None));

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Control block bit0 — set by the reader while a capture session is active.
const READER_ACTIVE: u32 = 0x1;
/// Control block mapping size (fixed; struct is 48 bytes).
const INFO_SIZE: usize = 64;
/// Create the PID-scoped named control block + frame event and map the control block.
/// Best-effort & non-fatal: if anything fails, capture simply stays off and the existing
/// frame-timing feature keeps working. Called from `attach()`.
pub fn capture_init() {
    let pid = unsafe { GetCurrentProcessId() };
    crate::debug_print(format!("capture: initialization started for pid {pid}"));
    unsafe {
        let info_name = wide(&format!("GlorpCaptureInfo_{pid}"));
        let event_name = wide(&format!("GlorpCaptureFrame_{pid}"));

        let mapping = match CreateFileMappingW(INVALID_HANDLE_VALUE, None, PAGE_READWRITE, 0, INFO_SIZE as u32, PCWSTR(info_name.as_ptr())) {
            Ok(m) => m,
            Err(e) => {
                crate::debug_print(format!("capture: CreateFileMappingW failed: {e}"));
                return;
            }
        };

        let view = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, INFO_SIZE);
        if view.Value.is_null() {
            crate::debug_print("capture: MapViewOfFile failed");
            let _ = CloseHandle(mapping);
            return;
        }

        // Zero, then stamp the producer-owned header fields.
        std::ptr::write_bytes(view.Value as *mut u8, 0, INFO_SIZE);
        let info = view.Value as *mut GlorpCaptureInfo;
        (*info).magic = MAGIC;
        (*info).version = VERSION;

        let frame_event = match CreateEventW(None, false, false, PCWSTR(event_name.as_ptr())) {
            Ok(e) => e,
            Err(e) => {
                crate::debug_print(format!("capture: CreateEventW failed: {e}"));
                return;
            }
        };

        INFO_PTR.store(view.Value as u64, Ordering::Release);

        *CAPTURE.lock().unwrap() = Some(Capture {
            pid,
            mapping: mapping.0 as usize,
            frame_event: frame_event.0 as usize,
            shared_handle: None,
            device: None,
            context: None,
            shared_tex: None,
            cached_w: 0,
            cached_h: 0,
            cached_format: 0,
        });

        crate::debug_print(format!("capture: initialized, pid {pid}"));
    }
}

/// Called from the `CreateSwapChainForComposition` hook each time a (new) swap chain appears, so
/// we learn the real D3D11 device and invalidate the shared texture for lazy re-creation against
/// the current swap chain. `device` is `None` if the device could not be obtained here; the
/// present path will lazily re-fetch it from the swap chain.
pub fn capture_on_swapchain(_swapchain: *mut c_void, device: Option<ID3D11Device>) {
    if INFO_PTR.load(Ordering::Acquire) == 0 {
        return;
    }
    if let Ok(mut guard) = CAPTURE.lock()
        && let Some(c) = guard.as_mut()
    {
        crate::debug_print(format!("capture: swap chain changed, device available: {}", device.is_some()));
        c.release_shared();
        if device.is_some() {
            c.device = device;
            c.context = None;
        }
    }
}

/// Ensure a shared texture matching (`w`, `h`, `format`) exists on the real device, then publish
/// dims/format in the control block. Currently holding the state lock.
fn ensure_shared_tex(c: &mut Capture, w: u32, h: u32, format: u32) {
    if let Some(_tex) = c.shared_tex.as_ref()
        && c.cached_w == w
        && c.cached_h == h
        && c.cached_format == format
    {
        return;
    }

    c.release_shared();

    let Some(device) = c.device.as_ref() else {
        crate::debug_print("capture: shared texture creation deferred because no D3D11 device is available");
        return;
    };

    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT(format as i32),
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0) as u32,
    };
    let mut tex: Option<ID3D11Texture2D> = None;
    if unsafe { device.CreateTexture2D(&desc, None, Some(&mut tex)) }.is_err() {
        crate::debug_print("capture: CreateTexture2D failed");
        return;
    }
    let Some(tex) = tex else {
        crate::debug_print("capture: CreateTexture2D succeeded without returning a texture");
        return;
    };

    // Publish a name so the OBS process can open it by name (raw handles never cross processes).
    let tex_name = wide(&format!("GlorpCaptureTex_{}", c.pid));
    let res: IDXGIResource1 = match tex.cast() {
        Ok(r) => r,
        Err(error) => {
            crate::debug_print(format!("capture: texture cast to IDXGIResource1 failed: {error}"));
            return;
        }
    };
    match unsafe { res.CreateSharedHandle(None, GENERIC_ALL.0, PCWSTR(tex_name.as_ptr())) } {
        Ok(h) => {
            c.shared_handle = Some(h.0 as usize);
        }
        Err(e) => {
            crate::debug_print(format!("capture: CreateSharedHandle failed: {e}"));
            return;
        }
    }

    c.shared_tex = Some(tex);
    c.cached_w = w;
    c.cached_h = h;
    c.cached_format = format;

    crate::debug_print(format!("capture: shared texture ready {w}x{h}, format {format}"));

    // Publish dims/format so the reader knows what to open.
    unsafe {
        let info_ptr = INFO_PTR.load(Ordering::Acquire) as *mut GlorpCaptureInfo;
        if !info_ptr.is_null() {
            (*info_ptr).width = w;
            (*info_ptr).height = h;
            (*info_ptr).format = format;
        }
    }
}

/// Called from `present_hk` (after the original present, so the buffer is stable). Gated on
/// READER_ACTIVE: when no reader is attached this returns almost immediately.
pub fn capture_on_present(swapchain: *mut c_void) {
    let info_ptr = INFO_PTR.load(Ordering::Acquire);
    if info_ptr == 0 {
        return;
    }
    // Fast path — no reader attached: zero capture work.
    unsafe {
        if (*(info_ptr as *const GlorpCaptureInfo)).flags & READER_ACTIVE == 0 {
            return;
        }
    }

    if let Ok(mut guard) = CAPTURE.lock() {
        let Some(c) = guard.as_mut() else { return };

        // Learn active dims/format from the live back buffer. We borrow (not own) the WebView2
        // swap-chain pointer so we never release a reference that isn't ours.
        let Some(sc) = (unsafe { IDXGISwapChain1::from_raw_borrowed(&swapchain) }) else {
            return;
        };
        let Ok(back) = (unsafe { sc.GetBuffer::<ID3D11Texture2D>(0) }) else {
            return;
        };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { back.GetDesc(&mut desc) };
        let w = desc.Width;
        let h = desc.Height;
        let fmt = desc.Format.0 as u32;

        // Device must be the real one; lazily fetch from the swap chain if not yet known.
        if c.device.is_none()
            && let Ok(dev) = unsafe { sc.GetDevice::<ID3D11Device>() }
        {
            c.device = Some(dev);
            c.context = None;
        }

        // Ensure the dedicated shared texture exists / matches current dims & format. Called
        // before taking any field borrow so it may take the whole `&mut Capture`.
        if c.shared_tex.is_none() || c.cached_w != w || c.cached_h != h || c.cached_format != fmt {
            ensure_shared_tex(c, w, h, fmt);
        }

        let Some(device) = c.device.as_ref() else { return };
        let Some(shared) = c.shared_tex.as_ref() else { return };
        if c.context.is_none()
            && let Ok(ctx) = unsafe { device.GetImmediateContext() }
        {
            c.context = Some(ctx);
        }
        let Some(context) = c.context.as_ref() else { return };

        // Copy the back buffer into the dedicated shared texture (synchronous on this device).
        unsafe { context.CopyResource(shared, &back) };

        // Signal a fresh, safe-to-copy frame and advance the counter.
        unsafe {
            let info = &mut *(info_ptr as *mut GlorpCaptureInfo);
            info.frame_counter = info.frame_counter.wrapping_add(1);
            let _ = SetEvent(HANDLE(c.frame_event as *mut _));
        }
    }
}

/// Best-effort cleanup on `DLL_PROCESS_DETACH`. Dropping the state closes the kernel handles and
/// releases the COM objects; named objects vanish when their last handle closes (process exit too).
pub fn capture_cleanup() {
    crate::debug_print("capture: cleanup started");
    if let Ok(mut guard) = CAPTURE.lock() {
        *guard = None;
    }
    INFO_PTR.store(0, Ordering::Release);
    crate::debug_print("capture: cleanup completed");
}
