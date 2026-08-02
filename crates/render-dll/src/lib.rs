use minhook::MinHook;
use std::{
    cell,
    collections::HashMap,
    ffi::c_void,
    mem,
    sync::{
        LazyLock, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};
use windows::Win32::{
    Foundation::*,
    Graphics::{
        Direct3D::*,
        Direct3D11::*,
        Dxgi::{Common::*, *},
    },
    System::{
        Diagnostics::Debug::*,
        Memory::*,
        SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
        Threading::*,
    },
};
use windows::core::*;

mod capture;

// 24 bytes for SharedState
#[repr(C)]
struct SharedState {
    frame_ns: u64, // 8 bytes
    fps: u64,      // 8 bytes
    // another 8 bytes free
    target_fps: u64, // 8 bytes
}

pub(crate) fn debug_print<T: AsRef<str>>(msg: T) {
    let mut wide: Vec<u16> = msg.as_ref().encode_utf16().collect();
    wide.push(0);
    unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())) };
}

fn get_idxgi() -> Result<(IDXGIFactory2, IDXGISwapChain1)> {
    unsafe {
        // make a dummy factory and dummy swap chain for the vtable
        let mut device: Option<ID3D11Device> = None;

        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_SINGLETHREADED,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;

        let device = device.unwrap();

        let dxgi_device: IDXGIDevice = device.cast()?;
        let dxgi_adapter: IDXGIAdapter = dxgi_device.GetAdapter()?;
        let factory: IDXGIFactory2 = dxgi_adapter.GetParent()?;

        let swap_chain: IDXGISwapChain1 = factory.CreateSwapChainForComposition(
            &dxgi_device,
            &DXGI_SWAP_CHAIN_DESC1 {
                Width: 1,
                Height: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: BOOL(0),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: 0,
            },
            None,
        )?;

        Ok((factory, swap_chain))
    }
}

#[allow(clippy::type_complexity)]
static mut ORIGINAL_CREATE_SWAPCHAIN: Option<unsafe fn(*mut c_void, *mut c_void, *const DXGI_SWAP_CHAIN_DESC1, *mut c_void, *mut *mut c_void) -> HRESULT> =
    None;

#[allow(clippy::type_complexity)]
static mut ORIGINAL_PRESENT: Option<unsafe fn(*mut c_void, u32, DXGI_PRESENT, *const DXGI_PRESENT_PARAMETERS) -> HRESULT> = None;

static WAIT_HANDLE: LazyLock<RwLock<HashMap<usize, usize>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
static SHARED_MEM_PTR: AtomicU64 = AtomicU64::new(0);
static MISSING_TIMING_MAPPING_LOGGED: AtomicBool = AtomicBool::new(false);

fn attach() {
    debug_print(format!("render: attach started, pid={}", unsafe { GetCurrentProcessId() }));
    unsafe {
        capture::capture_init();
        match OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, w!("GlorpFrameTiming")) {
            Ok(mapping) => {
                debug_print(format!("render: opened frame timing mapping={mapping:?}"));
                // 24 bytes for SharedState
                let ptr = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, 24);
                if !ptr.Value.is_null() {
                    SHARED_MEM_PTR.store(ptr.Value as u64, Ordering::Release);
                    debug_print(format!("render: mapped frame timing state={:?}", ptr.Value));
                } else {
                    debug_print("render: MapViewOfFile for frame timing returned null");
                }
            }
            Err(error) => debug_print(format!("render: failed to open frame timing mapping: {error}")),
        }
        debug_print("render: creating dummy D3D11 objects for hook discovery");
        let (factory, swap_chain) = get_idxgi().unwrap_or_else(|e| {
            debug_print(format!("Failed to get factory and swap chain: {:?}", e));
            panic!("Failed to get factory and swap chain");
        });

        let original_create_swapchain = MinHook::create_hook(
            factory.vtable().CreateSwapChainForComposition as *mut c_void,
            create_swapchain_hk as *mut c_void,
        )
        .unwrap_or_else(|e| {
            debug_print(format!("render: CreateSwapChainForComposition hook failed: {e:?}"));
            panic!("CreateSwapChainForComposition hook failed")
        });
        debug_print(format!("render: swap-chain hook created, trampoline={original_create_swapchain:p}"));

        let original_present = MinHook::create_hook(swap_chain.vtable().Present1 as *mut c_void, present_hk as *mut c_void).unwrap_or_else(|e| {
            debug_print(format!("render: Present1 hook failed: {e:?}"));
            panic!("Present1 hook failed")
        });
        debug_print(format!("render: Present1 hook created, trampoline={original_present:p}"));

        match MinHook::enable_all_hooks() {
            Ok(()) => debug_print("render: all MinHook hooks enabled"),
            Err(error) => debug_print(format!("render: cannot enable hooks: {error:?}")),
        }
        #[allow(clippy::missing_transmute_annotations)]
        {
            ORIGINAL_CREATE_SWAPCHAIN = mem::transmute(original_create_swapchain);
            ORIGINAL_PRESENT = mem::transmute(original_present);
        }
        debug_print("render: attach completed");
    }
}

unsafe extern "system" fn create_swapchain_hk(
    this: *mut c_void,
    pdevice: *mut c_void,
    pdesc: *const DXGI_SWAP_CHAIN_DESC1,
    prestricttooutput: *mut c_void,
    ppswapchain: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        debug_print(format!(
            "render: CreateSwapChainForComposition called original={}x{} buffers={} format={} flags={:#x}",
            (*pdesc).Width,
            (*pdesc).Height,
            (*pdesc).BufferCount,
            (*pdesc).Format.0,
            (*pdesc).Flags
        ));
        let mut desc = *pdesc;
        desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
        desc.BufferCount = 2; // 2 is the minimum
        desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL; // discard crashes
        desc.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
        // desc.Scaling = DXGI_SCALING_NONE; // this crashes
        desc.Flags = (DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 | DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0) as u32;

        let original_fn = ORIGINAL_CREATE_SWAPCHAIN.unwrap();

        let result = original_fn(this, pdevice, &desc, prestricttooutput, ppswapchain);
        if let Err(e) = result.ok() {
            debug_print(format!("Failed to create swap chain: {:#X} - {}", result.0, e));
            panic!("h");
        } else {
            debug_print(format!("render: swap chain created pointer={:?}", *ppswapchain));
            let swap_chain = IDXGISwapChain1::from_raw(*ppswapchain);
            // Learn the real D3D11 device from this swap chain and hand it to the capture module
            // (it invalidates any shared texture so it is re-created against this swap chain).
            let device = match swap_chain.GetDevice::<ID3D11Device>() {
                Ok(device) => {
                    debug_print("render: acquired D3D11 device from swap chain");
                    Some(device)
                }
                Err(error) => {
                    debug_print(format!("render: failed to acquire swap-chain D3D11 device: {error}"));
                    None
                }
            };
            capture::capture_on_swapchain(*ppswapchain, device);
            if let Ok(swap_chain2) = swap_chain.cast::<IDXGISwapChain2>() {
                swap_chain2
                    .SetMaximumFrameLatency(1)
                    .unwrap_or_else(|e| debug_print(format!("Failed to set latency: {:?}", e)));

                let waitable_obj = swap_chain2.GetFrameLatencyWaitableObject();
                debug_print(format!("render: frame-latency waitable object={waitable_obj:?}"));
                {
                    let mut guard = WAIT_HANDLE.write().unwrap();
                    guard.insert(*ppswapchain as usize, waitable_obj.0 as usize);
                }

                // don't release
                mem::forget(swap_chain2);
            } else {
                debug_print("render: swap chain does not expose IDXGISwapChain2");
            }
            result
        }
    }
}

#[link(name = "Avrt")]
unsafe extern "system" {
    fn AvSetMmThreadCharacteristicsW(task_name: PCWSTR, task_index: *mut u32) -> HANDLE;
}

unsafe extern "system" fn present_hk(
    p_this: *mut c_void,
    sync_interval: u32,
    mut present_flags: DXGI_PRESENT,
    p_present_parameters: *const DXGI_PRESENT_PARAMETERS,
) -> HRESULT {
    let ptr = SHARED_MEM_PTR.load(Ordering::Acquire);
    if ptr == 0 {
        if !MISSING_TIMING_MAPPING_LOGGED.swap(true, Ordering::Relaxed) {
            debug_print("render: Present1 running without GlorpFrameTiming mapping; timing, limiter, and capture path bypassed");
        }
        unsafe {
            let original_present = ORIGINAL_PRESENT.unwrap();
            return original_present(p_this, sync_interval, present_flags, p_present_parameters);
        }
    }

    thread_local! {
        static INITIALIZED: cell::Cell<bool> = const { cell::Cell::new(false) };
        static LAST_PRESENT: cell::Cell<Option<std::time::Instant>> = const { cell::Cell::new(None) };
        static FRAME_NS_EMA: cell::Cell<u64> = const { cell::Cell::new(0) };
        static LAST_REPORTED_MOVE_QPC: cell::Cell<i64> = const { cell::Cell::new(0) };
        static DIAGNOSTIC_START: cell::Cell<Option<std::time::Instant>> = const { cell::Cell::new(None) };
        static DIAGNOSTIC_PRESENTS: cell::Cell<u64> = const { cell::Cell::new(0) };
        static DIAGNOSTIC_MAX_FRAME_NS: cell::Cell<u64> = const { cell::Cell::new(0) };

        // pacing clock for the last frame, for fps limiter
        static LIMIT_CLOCK: cell::Cell<Option<std::time::Instant>> = const { cell::Cell::new(None) };

        // cached waitable handle
        // populated once per thread on first present
        static CACHED_WAIT_HANDLE: cell::Cell<Option<usize>> = const { cell::Cell::new(None) };
    }
    if !INITIALIZED.get() {
        let mut task_index = 0u32;
        let avrt_handle = unsafe { AvSetMmThreadCharacteristicsW(w!("Games"), &mut task_index) };
        debug_print(format!(
            "render: present thread initialized id={} avrt_handle={avrt_handle:?} task_index={task_index}",
            unsafe { GetCurrentThreadId() }
        ));
        INITIALIZED.set(true);
    }

    LAST_PRESENT.with(|last| {
        let now = std::time::Instant::now();
        if let Some(prev) = last.get() {
            let frame_ns = now.duration_since(prev).as_nanos() as u64;
            DIAGNOSTIC_MAX_FRAME_NS.set(DIAGNOSTIC_MAX_FRAME_NS.get().max(frame_ns));
            FRAME_NS_EMA.with(|avg| {
                let next = if avg.get() == 0 { frame_ns } else { (avg.get() * 31 + frame_ns) / 32 };
                avg.set(next);
                unsafe {
                    let shared = &mut *(ptr as *mut SharedState);
                    let fps = 1_000_000_000 / next;

                    shared.frame_ns = next;
                    shared.fps = fps;
                }
            });
        }

        last.set(Some(now));
        DIAGNOSTIC_PRESENTS.set(DIAGNOSTIC_PRESENTS.get().wrapping_add(1));
        let diagnostic_start = DIAGNOSTIC_START.get().unwrap_or_else(|| {
            DIAGNOSTIC_START.set(Some(now));
            now
        });
        if now.duration_since(diagnostic_start) >= std::time::Duration::from_secs(10) {
            let ema_ns = FRAME_NS_EMA.get();
            let ema_fps = 1_000_000_000u64.checked_div(ema_ns).unwrap_or(0);
            let target_fps = unsafe { (*(ptr as *const SharedState)).target_fps };
            let has_wait_handle = WAIT_HANDLE.read().unwrap().contains_key(&(p_this as usize));
            debug_print(format!(
                "render: 10s stats presents={} ema_fps={} ema_ms={:.3} max_frame_ms={:.3} target_fps={} wait_handle={}",
                DIAGNOSTIC_PRESENTS.get(),
                ema_fps,
                ema_ns as f64 / 1_000_000.0,
                DIAGNOSTIC_MAX_FRAME_NS.get() as f64 / 1_000_000.0,
                target_fps,
                has_wait_handle
            ));
            DIAGNOSTIC_START.set(Some(now));
            DIAGNOSTIC_PRESENTS.set(0);
            DIAGNOSTIC_MAX_FRAME_NS.set(0);
        }
    });

    unsafe {
        let handle_opt = CACHED_WAIT_HANDLE.with(|cached| {
            if let Some(h) = cached.get() {
                return Some(h);
            }
            // First call on this thread
            let h = WAIT_HANDLE.read().unwrap().get(&(p_this as usize)).copied();
            if let Some(h) = h {
                cached.set(Some(h));
                debug_print(format!("render: cached frame-latency wait handle={h:#x} swapchain={p_this:?}"));
            }
            h
        });

        if let Some(h_raw) = handle_opt {
            let h = HANDLE(h_raw as *mut _);
            let _ = WaitForSingleObjectEx(h, 0, false);
        }

        // limiter
        let target_fps = (*(ptr as *const SharedState)).target_fps;
        if let Some(nanos) = 1_000_000_000u64.checked_div(target_fps) {
            let target_frame_time = std::time::Duration::from_nanos(nanos);
            LIMIT_CLOCK.with(|last| {
                let now = std::time::Instant::now();
                if let Some(prev) = last.get() {
                    let elapsed = now.duration_since(prev);
                    if elapsed < target_frame_time {
                        let remaining = target_frame_time - elapsed;
                        // sleep for the bulk, spin the last ~1ms for accuracy
                        if remaining > std::time::Duration::from_millis(2) {
                            thread::sleep(remaining - std::time::Duration::from_millis(1));
                        }
                        while prev.elapsed() < target_frame_time {
                            std::hint::spin_loop();
                        }
                    }
                }
                last.set(Some(std::time::Instant::now()));
            });
        }
        // end of limiter

        if sync_interval == 0 {
            present_flags |= DXGI_PRESENT_ALLOW_TEARING;
        }
        let original_present = ORIGINAL_PRESENT.unwrap();
        let mut hr = original_present(p_this, sync_interval, present_flags, p_present_parameters);
        if hr.is_err() && (present_flags.0 & DXGI_PRESENT_ALLOW_TEARING.0) != 0 {
            debug_print(format!(
                "Present failed with tearing flag: {:#X}, retrying fallback",
                hr.0
            ));
            let fallback_flags = DXGI_PRESENT(present_flags.0 & !DXGI_PRESENT_ALLOW_TEARING.0);
            hr = original_present(p_this, sync_interval, fallback_flags, p_present_parameters);
            debug_print(format!("render: Present fallback result={:#X}", hr.0));
        }

        // OBS capture (producer side): run after the original present so the back buffer is
        // stable before the CopyResource — gated internally on READER_ACTIVE.
        capture::capture_on_present(p_this);

        hr
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(_: HINSTANCE, call_reason: u32, _: *mut ()) {
    if call_reason == DLL_PROCESS_ATTACH {
        debug_print("render: DLL_PROCESS_ATTACH, spawning initialization thread");
        thread::spawn(|| {
            attach();
        });
    } else if call_reason == DLL_PROCESS_DETACH {
        debug_print("render: DLL_PROCESS_DETACH, cleaning capture state");
        capture::capture_cleanup();
    }
}
