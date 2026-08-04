//! Minimal, bindings to the OBS (libobs) public C API surface.
//!
//! OBS ships **no import library and no installed headers**, so we resolve every function at
//! runtime from the already-resident `obs.dll` via `GetProcAddress`. There is no link-time

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use std::{
    ffi::{c_char, c_void},
    mem,
    sync::LazyLock,
};
use windows::{
    Win32::{Foundation::*, System::LibraryLoader::*},
    core::*,
};

// ---- constants (from obs.h / graphics.h) ----
pub const OBS_SOURCE_TYPE_INPUT: u32 = 0;
pub const OBS_SOURCE_VIDEO: u32 = 1 << 0;
pub const GS_DEVICE_DIRECT3D_11: i32 = 2;

// ---- opaque handles (never dereferenced here) ----
#[repr(C)]
pub struct gs_texture_t {
    _p: [u8; 0],
}
#[repr(C)]
pub struct gs_effect_t {
    _p: [u8; 0],
}
#[repr(C)]
pub struct gs_eparam_t {
    _p: [u8; 0],
}
#[repr(C)]
pub struct obs_data_t {
    _p: [u8; 0],
}
#[repr(C)]
pub struct obs_source_t {
    _p: [u8; 0],
}

// ---- callback typedefs ----
pub type GetNameFn = unsafe extern "C" fn(data: *mut c_void) -> *const c_char;
pub type CreateFn = unsafe extern "C" fn(settings: *mut obs_data_t, source: *mut obs_source_t) -> *mut c_void;
pub type DestroyFn = unsafe extern "C" fn(data: *mut c_void);
pub type GetSizeFn = unsafe extern "C" fn(data: *mut c_void) -> u32;
pub type VideoTickFn = unsafe extern "C" fn(data: *mut c_void, seconds: f32);
pub type VideoRenderFn = unsafe extern "C" fn(data: *mut c_void, effect: *mut gs_effect_t);

/// `struct obs_source_info` from obs-source.h — a faithful, sorted slice of the real struct
/// through `video_render`. We register with `obs_register_source_s(info, sizeof)` and OBS only
/// reads up to `sizeof`, so everything after `video_render` can be omitted.
#[repr(C)]
pub struct obs_source_info {
    pub id: *const c_char,
    pub source_type: u32,
    pub output_flags: u32,
    pub get_name: Option<GetNameFn>,
    pub create: Option<CreateFn>,
    pub destroy: Option<DestroyFn>,
    pub get_width: Option<GetSizeFn>,
    pub get_height: Option<GetSizeFn>,
    pub get_defaults: Option<unsafe extern "C" fn(settings: *mut obs_data_t)>,
    pub get_properties: Option<unsafe extern "C" fn(data: *mut c_void) -> *mut c_void>,
    pub update: Option<unsafe extern "C" fn(data: *mut c_void, settings: *mut obs_data_t)>,
    pub activate: Option<unsafe extern "C" fn(data: *mut c_void)>,
    pub deactivate: Option<unsafe extern "C" fn(data: *mut c_void)>,
    pub show: Option<unsafe extern "C" fn(data: *mut c_void)>,
    pub hide: Option<unsafe extern "C" fn(data: *mut c_void)>,
    pub video_tick: Option<VideoTickFn>,
    pub video_render: Option<VideoRenderFn>,
}

/// Runtime-resolved function pointers into `obs.dll`.
pub struct ObsApi {
    pub register_source: unsafe extern "C" fn(info: *const obs_source_info, size: usize),
    pub gs_get_device_type: unsafe extern "C" fn() -> i32,
    pub gs_get_device_obj: unsafe extern "C" fn() -> *mut c_void,
    pub gs_texture_wrap_obj: unsafe extern "C" fn(obj: *mut c_void) -> *mut gs_texture_t,
    pub gs_texture_destroy: unsafe extern "C" fn(tex: *mut gs_texture_t),
    pub gs_effect_get_param_by_name: unsafe extern "C" fn(effect: *mut gs_effect_t, name: *const c_char) -> *mut gs_eparam_t,
    pub gs_effect_set_texture: unsafe extern "C" fn(param: *mut gs_eparam_t, tex: *mut gs_texture_t),
    pub gs_draw_sprite: unsafe extern "C" fn(tex: *mut gs_texture_t, flip: u32, cx: u32, cy: u32),
}

/// Resolved once: points into the already-loaded `obs.dll`.
pub static OBS: LazyLock<Option<ObsApi>> = LazyLock::new(resolve_obs_api);

unsafe fn resolve<T: Copy>(module: HMODULE, name: &[u8]) -> Option<T> {
    let p = GetProcAddress(module, PCSTR(name.as_ptr()))?;
    // FARPROC (8 bytes) -> the requested fn-pointer type (also 8 bytes).
    Some(mem::transmute_copy::<_, T>(&p))
}

fn resolve_obs_api() -> Option<ObsApi> {
    unsafe {
        let module = GetModuleHandleW(w!("obs.dll")).ok()?;
        Some(ObsApi {
            register_source: resolve(module, b"obs_register_source_s\0")?,
            gs_get_device_type: resolve(module, b"gs_get_device_type\0")?,
            gs_get_device_obj: resolve(module, b"gs_get_device_obj\0")?,
            gs_texture_wrap_obj: resolve(module, b"gs_texture_wrap_obj\0")?,
            gs_texture_destroy: resolve(module, b"gs_texture_destroy\0")?,
            gs_effect_get_param_by_name: resolve(module, b"gs_effect_get_param_by_name\0")?,
            gs_effect_set_texture: resolve(module, b"gs_effect_set_texture\0")?,
            gs_draw_sprite: resolve(module, b"gs_draw_sprite\0")?,
        })
    }
}
