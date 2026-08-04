//! OBS source plugin — "Glorp Capture" (consumer side of the shared-texture capture pair).
//!
//! Loaded by OBS into `obs64.exe`. In `obs_module_load` we register an input video source that:
//!   1. discovers the producer (the WebView2 GPU process running `render.dll`) via §10 scan,
//!   2. opens the producer's named shared texture on OBS's own D3D11 device
//!      (`gs_get_device_obj` -> `ID3D11Device1::OpenSharedResourceByName` -> `gs_texture_wrap_obj`),
//!   3. renders it each video frame with the stock draw effect (zero-copy, GPU-only).
//!
//! Discovery + reopening are driven from `video_tick`, so the source also self-heals when the
//! GPU process is respawned under a new PID (§11).

#![allow(non_snake_case, unsafe_op_in_unsafe_fn)]

mod capture;
mod obsabi;

use std::{
    ffi::{c_char, c_void},
    sync::atomic::{AtomicI8, Ordering},
    time::{Duration, Instant},
};
use obsabi::{OBS, ObsApi, gs_effect_t, gs_texture_t, obs_data_t, obs_source_info, obs_source_t};
use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        Foundation::GENERIC_ALL,
        Graphics::Direct3D11::{ID3D11Device, ID3D11Device1, ID3D11Texture2D},
        System::Diagnostics::Debug::OutputDebugStringW,
    },
};

const DISCOVER_RETRY: Duration = Duration::from_millis(500);
const STALL_RETRY: Duration = Duration::from_secs(1);

/// Cache of whether the OBS graphics backend is D3D11 (-1 unknown, 0 no, 1 yes).
/// This is only meaningful once a gs context is active, i.e. from `video_tick`/`video_render` —
/// NOT from `obs_module_load`, where the main thread has no gs context yet.
static BACKEND_OK: AtomicI8 = AtomicI8::new(-1);

fn backend_supported(api: &ObsApi) -> bool {
    match BACKEND_OK.load(Ordering::Relaxed) {
        1 => return true,
        0 => return false,
        _ => {}
    }
    let ok = unsafe { (api.gs_get_device_type)() } == obsabi::GS_DEVICE_DIRECT3D_11;
    BACKEND_OK.store(if ok { 1 } else { 0 }, Ordering::Relaxed);
    if !ok {
        debug_print("capture: only D3D11 backend supported");
    }
    ok
}

fn debug_print(msg: impl AsRef<str>) {
    let mut wide: Vec<u16> = msg.as_ref().encode_utf16().collect();
    wide.push(0);
    unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())) };
}

struct GlorpSource {
    session: Option<capture::Session>,
    dxgi_tex: Option<ID3D11Texture2D>,
    gs_tex: *mut gs_texture_t,
    width: u32,
    height: u32,
    last_counter: u64,
    last_attempt: Instant,
}

impl GlorpSource {
    fn new() -> Self {
        Self {
            session: None,
            dxgi_tex: None,
            gs_tex: std::ptr::null_mut(),
            width: 0,
            height: 0,
            last_counter: 0,
            last_attempt: Instant::now(),
        }
    }

    fn teardown_texture(&mut self) {
        if let Some(api) = OBS.as_ref() {
            if !self.gs_tex.is_null() {
                unsafe { (api.gs_texture_destroy)(self.gs_tex) };
                self.gs_tex = std::ptr::null_mut();
            }
        }
        self.dxgi_tex = None;
        self.width = 0;
        self.height = 0;
    }

    /// Open the producer's named shared texture on OBS's device and wrap it into a `gs_texture_t`.
    /// No-op if one already exists at the given size.
    fn open_texture(&mut self, w: u32, h: u32) {
        if !self.gs_tex.is_null() && self.width == w && self.height == h {
            return;
        }
        self.teardown_texture();

        let Some(api) = OBS.as_ref() else { return };
        if !backend_supported(api) {
            return;
        }
        let Some(sess) = self.session.as_ref() else { return };

        unsafe {
            let dev_raw = (api.gs_get_device_obj)();
            if dev_raw.is_null() {
                return;
            }
            // Borrowed pointer — do NOT release what we don't own.
            let Some(dev) = ID3D11Device::from_raw_borrowed(&dev_raw) else { return };
            // OpenSharedResourceByName lives on ID3D11Device1 (runtime 11.1+, always available).
            let Ok(dev1) = dev.cast::<ID3D11Device1>() else { return };

            let name = format!("GlorpCaptureTex_{}", sess.pid);
            let name_w: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            let opened = dev1
                .OpenSharedResourceByName::<_, ID3D11Texture2D>(PCWSTR(name_w.as_ptr()), GENERIC_ALL.0);
            drop(dev1); // release our QI reference

            let Ok(tex) = opened else {
                debug_print(format!("capture: OpenSharedResourceByName failed for {name}"));
                return;
            };
            let g = (api.gs_texture_wrap_obj)(Interface::as_raw(&tex));
            if g.is_null() {
                return; // `tex` drops & releases; nothing leaked
            }
            self.dxgi_tex = Some(tex); // keep object alive for the lifetime of `gs_tex`
            self.gs_tex = g;
            self.width = w;
            self.height = h;
            debug_print(format!("capture: opened {name} {w}x{h}"));
        }
    }
}

// ---- OBS callbacks ----

unsafe extern "C" fn get_name(_data: *mut c_void) -> *const c_char {
    c"Glorp Capture".as_ptr()
}

unsafe extern "C" fn create(_settings: *mut obs_data_t, _source: *mut obs_source_t) -> *mut c_void {
    Box::into_raw(Box::new(GlorpSource::new())) as *mut c_void
}

unsafe extern "C" fn destroy(data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let s = &mut *(data as *mut GlorpSource);
    if let Some(mut sess) = s.session.take() {
        capture::set_reader_active(&sess, false);
        debug_print(format!("capture: reader off (pid {})", sess.pid));
        sess.close();
    }
    s.teardown_texture();
    drop(Box::from_raw(data as *mut GlorpSource));
}

unsafe extern "C" fn get_width(data: *mut c_void) -> u32 {
    (*(data as *mut GlorpSource)).width
}

unsafe extern "C" fn get_height(data: *mut c_void) -> u32 {
    (*(data as *mut GlorpSource)).height
}

unsafe extern "C" fn video_tick(data: *mut c_void, _seconds: f32) {
    let s = &mut *(data as *mut GlorpSource);

    // No session yet -> try to find the producer (throttled).
    if s.session.is_none() {
        if s.last_attempt.elapsed() >= DISCOVER_RETRY {
            s.last_attempt = Instant::now();
            if let Some(sess) = capture::discover() {
                capture::set_reader_active(&sess, true);
                debug_print(format!("capture: reader on (pid {})", sess.pid));
                s.session = Some(sess);
            }
        }
        return;
    }

    let sess = s.session.as_ref().unwrap();
    let frame_counter = unsafe { (*sess.info).frame_counter };

    // Producer stalled (no new frame)?
    if frame_counter == s.last_counter {
        // If the GPU process itself is gone, tear everything down and re-discover under the new
        // PID. If it's merely paused (game minimized / frozen), keep showing the last frame.
        if s.last_attempt.elapsed() >= STALL_RETRY && !capture::process_exists(sess.pid) {
            let mut old = s.session.take().unwrap();
            capture::set_reader_active(&old, false);
            old.close();
            s.teardown_texture();
            s.last_attempt = Instant::now();
        }
        return;
    }

    s.last_counter = frame_counter;
    s.last_attempt = Instant::now();
    // Texture (re)open happens in video_render, which has an active gs context — necessary for
    // gs_get_device_obj()/OpenSharedResourceByName/gs_texture_wrap_obj.
}

unsafe extern "C" fn video_render(data: *mut c_void, effect: *mut gs_effect_t) {
    let s = &mut *(data as *mut GlorpSource);

    // Open (or resize/reopen) the shared texture here, on the render thread, where a gs context
    // is guaranteed active.
    if let Some(sess) = s.session.as_ref() {
        let w = unsafe { (*sess.info).width };
        let h = unsafe { (*sess.info).height };
        if w != 0 && h != 0 && (s.gs_tex.is_null() || s.width != w || s.height != h) {
            s.open_texture(w, h);
        }
    }

    if s.gs_tex.is_null() {
        return;
    }
    let Some(api) = OBS.as_ref() else { return };
    let image = (api.gs_effect_get_param_by_name)(effect, c"image".as_ptr());
    if image.is_null() {
        return;
    }
    (api.gs_effect_set_texture)(image, s.gs_tex);
    (api.gs_draw_sprite)(s.gs_tex, 0, s.width, s.height);
}

// ---- plugin entry ----

#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load() -> bool {
    let Some(api) = OBS.as_ref() else {
        debug_print("capture: obs.dll API not resolvable");
        return false;
    };
    // NOTE: no gs backend check here — the main thread has no gs context while loading modules,
    // so gs_get_device_type() is unreliable here. backend_supported() is validated lazily on the
    // render thread (in open_texture) before the first device/texture work.

    let info = obs_source_info {
        id: c"glorp_capture".as_ptr(),
        source_type: obsabi::OBS_SOURCE_TYPE_INPUT,
        output_flags: obsabi::OBS_SOURCE_VIDEO,
        get_name: Some(get_name),
        create: Some(create),
        destroy: Some(destroy),
        get_width: Some(get_width),
        get_height: Some(get_height),
        get_defaults: None,
        get_properties: None,
        update: None,
        activate: None,
        deactivate: None,
        show: None,
        hide: None,
        video_tick: Some(video_tick),
        video_render: Some(video_render),
    };

    unsafe {
        (api.register_source)(&info, std::mem::size_of::<obs_source_info>());
    }
    debug_print("capture: Glorp Capture source registered");
    true
}

/// Required by OBS's module loader (equivalent of `OBS_DECLARE_MODULE`): receives the module's
/// `obs_module_t*`. We don't use `obs_current_module`/locale helpers, so we just accept it.
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_set_pointer(_module: *mut c_void) {}

/// Required by OBS's module loader: reports the libobs API version we were built against. OBS only
/// rejects plugins claiming a *newer* libobs than itself, so we target OBS 32.x (API 32.0.0).
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_ver() -> u32 {
    32 << 24 // LIBOBS_API_VER = 0x20000000 (major 32, minor 0, patch 0)
}


