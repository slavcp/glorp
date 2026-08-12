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
        Memory::*,
        SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
        Threading::*,
    },
};
use windows::core::*;

mod capture;

// Thread-safe wrapper for Win32 kernel handles
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
struct SendHandle(pub HANDLE);

unsafe impl Send for SendHandle {}
unsafe impl Sync for SendHandle {}

// 24 bytes for SharedState
#[repr(C)]
struct SharedState {
    frame_ns: u64, // 8 bytes
    fps: u64,      // 8 bytes
    // another 8 bytes free
    target_fps: u64, // 8 bytes
}

#[macro_export]
macro_rules! debug_print {
    ($($arg:tt)*) => {
        if cfg!(feature = "verbose-logs") {
            let msg = format!($($arg)*);
            let wide: Vec<u16> = msg.encode_utf16().chain(Some(0)).collect();
            #[allow(unused_unsafe)]
            unsafe {
                ::windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(
                    ::windows::core::PCWSTR(wide.as_ptr()),
                );
            }
        }
    };
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

static WAIT_HANDLE: LazyLock<RwLock<HashMap<usize, SendHandle>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
static WAIT_HANDLE_GENERATION: AtomicU64 = AtomicU64::new(0);

static SHARED_MEM_PTR: AtomicU64 = AtomicU64::new(0);
static MISSING_TIMING_MAPPING_LOGGED: AtomicBool = AtomicBool::new(false);

static GLOBAL_LIMIT_CLOCK: LazyLock<RwLock<Option<std::time::Instant>>> = LazyLock::new(|| RwLock::new(None));
static MAIN_SWAPCHAIN_CREATED: AtomicBool = AtomicBool::new(false);

fn attach() {
    debug_print!("render: attach started, pid={}", unsafe { GetCurrentProcessId() });
    unsafe {
        capture::capture_init();
        match OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, w!("GlorpFrameTiming")) {
            Ok(mapping) => {
                debug_print!("render: opened frame timing mapping={mapping:?}");
                // 24 bytes for SharedState
                let ptr = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, 24);
                if !ptr.Value.is_null() {
                    SHARED_MEM_PTR.store(ptr.Value as u64, Ordering::Release);
                    debug_print!("render: mapped frame timing state={:?}", ptr.Value);
                } else {
                    debug_print!("render: MapViewOfFile for frame timing returned null");
                }
            }
            Err(_error) => (),
        }
        debug_print!("render: creating dummy D3D11 objects for hook discovery");
        let (factory, swap_chain) = get_idxgi().unwrap_or_else(|e| {
            debug_print!("Failed to get factory and swap chain: {:?}", e);
            panic!("Failed to get factory and swap chain");
        });

        let original_create_swapchain = MinHook::create_hook(
            factory.vtable().CreateSwapChainForComposition as *mut c_void,
            create_swapchain_hk as *mut c_void,
        )
        .unwrap_or_else(|e| {
            debug_print!("render: CreateSwapChainForComposition hook failed: {e:?}");
            panic!("CreateSwapChainForComposition hook failed")
        });
        debug_print!("render: swap-chain hook created, trampoline={original_create_swapchain:p}");

        let original_present = MinHook::create_hook(swap_chain.vtable().Present1 as *mut c_void, present_hk as *mut c_void).unwrap_or_else(|e| {
            debug_print!("render: Present1 hook failed: {e:?}");
            panic!("Present1 hook failed")
        });
        debug_print!("render: Present1 hook created, trampoline={original_present:p}");

        match MinHook::enable_all_hooks() {
            Ok(()) => debug_print!("render: all MinHook hooks enabled"),
            Err(error) => debug_print!("render: cannot enable hooks: {error:?}"),
        }
        #[allow(clippy::missing_transmute_annotations)]
        {
            ORIGINAL_CREATE_SWAPCHAIN = mem::transmute(original_create_swapchain);
            ORIGINAL_PRESENT = mem::transmute(original_present);
        }
        debug_print!("render: attach completed");
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
        if (*pdesc).Width < 600 || (*pdesc).Height < 600 || MAIN_SWAPCHAIN_CREATED.load(Ordering::Acquire) {
            let original_fn = ORIGINAL_CREATE_SWAPCHAIN.unwrap();
            let result = original_fn(this, pdevice, pdesc, prestricttooutput, ppswapchain);

            // new swapchain creation can be on the same address as a destroyed one, so purge stale wait handle
            if result.is_ok()
                && !ppswapchain.is_null()
                && WAIT_HANDLE.write().unwrap().remove(&(*ppswapchain as usize)).is_some()
            {
                debug_print!("render: purged stale wait handle for reused swapchain address {:?}", *ppswapchain);
                // bump when WAIT_HANDLE changes so the per-thread caches in present_hk drop stale entries
                WAIT_HANDLE_GENERATION.fetch_add(1, Ordering::Release);
            }
            return result;
        }
        debug_print!(
            "render: CreateSwapChainForComposition called original={}x{} buffers={} format={} flags={:#x}",
            (*pdesc).Width,
            (*pdesc).Height,
            (*pdesc).BufferCount,
            (*pdesc).Format.0,
            (*pdesc).Flags
        );
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
            debug_print!("Failed to create swap chain: {:#X} - {}", result.0, e);
            panic!("h");
        } else {
            MAIN_SWAPCHAIN_CREATED.store(true, Ordering::Release);
            debug_print!("render: swap chain created pointer={:?}", *ppswapchain);
            let swap_chain = IDXGISwapChain1::from_raw(*ppswapchain);
            // Learn the real D3D11 device from this swap chain and hand it to the capture module
            // (it invalidates any shared texture so it is re-created against this swap chain).
            let device = match swap_chain.GetDevice::<ID3D11Device>() {
                Ok(device) => {
                    debug_print!("render: acquired D3D11 device from swap chain");
                    Some(device)
                }
                Err(error) => {
                    debug_print!("render: failed to acquire swap-chain D3D11 device: {error}");
                    None
                }
            };
            capture::capture_on_swapchain(*ppswapchain, device);
            if let Ok(swap_chain2) = swap_chain.cast::<IDXGISwapChain2>() {
                swap_chain2
                    .SetMaximumFrameLatency(1)
                    .unwrap_or_else(|e| debug_print!("Failed to set latency: {:?}", e));

                let waitable_obj = swap_chain2.GetFrameLatencyWaitableObject();
                debug_print!("render: frame-latency waitable object={waitable_obj:?}");
                {
                    let mut guard = WAIT_HANDLE.write().unwrap();
                    if let Some(old_handle) = guard.insert(*ppswapchain as usize, SendHandle(waitable_obj))
                        && !old_handle.0.is_invalid()
                    {
                        debug_print!("render: closing replaced swapchain wait handle={:?}", old_handle.0);
                        let _ = CloseHandle(old_handle.0);
                    }
                    WAIT_HANDLE_GENERATION.fetch_add(1, Ordering::Release);
                }

                // don't release
                mem::forget(swap_chain2);
            } else {
                debug_print!("render: swap chain does not expose IDXGISwapChain2");
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
            debug_print!("render: Present1 running without GlorpFrameTiming mapping; timing, limiter, and capture path bypassed");
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

        static CACHED_WAIT_HANDLES: std::cell::RefCell<HashMap<usize, SendHandle>> = std::cell::RefCell::new(HashMap::new());
        static CACHED_WAIT_GENERATION: cell::Cell<u64> = const { cell::Cell::new(0) };
        static WAIT_TIMEOUT_STREAK: cell::Cell<u32> = const { cell::Cell::new(0) };

        // per-thread perf accounting for wall-clock time matching help me debug, but can be removed
        static PERF_WINDOW_START: cell::Cell<Option<std::time::Instant>> = const { cell::Cell::new(None) };
        static PERF_PRESENTS: cell::Cell<u64> = const { cell::Cell::new(0) };
        static PERF_WAIT_NS: cell::Cell<u64> = const { cell::Cell::new(0) };
        static PERF_WAIT_MAX_NS: cell::Cell<u64> = const { cell::Cell::new(0) };
        static PERF_PRESENT_NS: cell::Cell<u64> = const { cell::Cell::new(0) };
        static PERF_PRESENT_MAX_NS: cell::Cell<u64> = const { cell::Cell::new(0) };
        static PERF_TIMEOUTS: cell::Cell<u64> = const { cell::Cell::new(0) };
    }
    if !INITIALIZED.get() {
        let mut task_index = 0u32;
        let avrt_handle = unsafe { AvSetMmThreadCharacteristicsW(w!("Games"), &mut task_index) };
        debug_print!(
            "render: present thread initialized id={} avrt_handle={avrt_handle:?} task_index={task_index}",
            unsafe { GetCurrentThreadId() }
        );
        INITIALIZED.set(true);
    }

    LAST_PRESENT.with(|last| {
        let now = std::time::Instant::now();
        if let Some(prev) = last.get() {
            let frame_ns = now.duration_since(prev).as_nanos() as u64;
            if cfg!(feature = "verbose-logs") {
                DIAGNOSTIC_MAX_FRAME_NS.set(DIAGNOSTIC_MAX_FRAME_NS.get().max(frame_ns));
            }
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
        if cfg!(feature = "verbose-logs") {
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
                debug_print!(
                    "render: 10s stats thread_id={} presents={} ema_fps={} ema_ms={:.3} max_frame_ms={:.3} target_fps={} wait_handle={}",
                    unsafe { GetCurrentThreadId() },
                    DIAGNOSTIC_PRESENTS.get(),
                    ema_fps,
                    ema_ns as f64 / 1_000_000.0,
                    DIAGNOSTIC_MAX_FRAME_NS.get() as f64 / 1_000_000.0,
                    target_fps,
                    has_wait_handle
                );
                DIAGNOSTIC_START.set(Some(now));
                DIAGNOSTIC_PRESENTS.set(0);
                DIAGNOSTIC_MAX_FRAME_NS.set(0);
            }
        }
    });

    unsafe {
        let generation = WAIT_HANDLE_GENERATION.load(Ordering::Acquire);
        let handle_opt = CACHED_WAIT_HANDLES.with(|cache| {
            let mut map = cache.borrow_mut();
            if CACHED_WAIT_GENERATION.get() != generation {
                map.clear();
                CACHED_WAIT_GENERATION.set(generation);
            }
            if let Some(&h) = map.get(&(p_this as usize)) {
                Some(h)
            } else {
                let h = WAIT_HANDLE.read().unwrap().get(&(p_this as usize)).copied();
                if let Some(h) = h {
                    map.insert(p_this as usize, h);
                }
                h
            }
        });

        let wait_started = std::time::Instant::now();
        if let Some(h) = handle_opt {
            // drop handle if the wait fails or times out repeatedly
            let wait_result = WaitForSingleObjectEx(h.0, 100, false);
            let drop_handle = if wait_result == WAIT_FAILED {
                true
            } else if wait_result == WAIT_TIMEOUT {
                if cfg!(feature = "verbose-logs") {
                    PERF_TIMEOUTS.set(PERF_TIMEOUTS.get() + 1);
                }
                let streak = WAIT_TIMEOUT_STREAK.get() + 1;
                WAIT_TIMEOUT_STREAK.set(streak);
                streak >= 3
            } else {
                WAIT_TIMEOUT_STREAK.set(0);
                false
            };
            if drop_handle {
                debug_print!("render: dropping unresponsive wait handle for swapchain {p_this:?} result={wait_result:?}");
                WAIT_TIMEOUT_STREAK.set(0);
                WAIT_HANDLE.write().unwrap().remove(&(p_this as usize));
                WAIT_HANDLE_GENERATION.fetch_add(1, Ordering::Release);
            }
        }
        let wait_ns = if cfg!(feature = "verbose-logs") {
            wait_started.elapsed().as_nanos() as u64
        } else {
            0
        };

        // limiter
        let target_fps = (*(ptr as *const SharedState)).target_fps;
        if let Some(nanos) = 1_000_000_000u64.checked_div(target_fps) {
            let target_frame_time = std::time::Duration::from_nanos(nanos);
            let now = std::time::Instant::now();
            let prev_opt = { *GLOBAL_LIMIT_CLOCK.read().unwrap() };

            if let Some(prev) = prev_opt {
                let elapsed = now.duration_since(prev);
                if elapsed < target_frame_time {
                    let remaining = target_frame_time - elapsed;
                    debug_print!(
                        "render: limiter active on thread_id={} sleeping {:.2}ms to maintain target_fps={}",
                        GetCurrentThreadId(),
                        remaining.as_secs_f64() * 1000.0,
                        target_fps
                    );
                    // sleep for the bulk, spin the last ~1ms for accuracy
                    if remaining > std::time::Duration::from_millis(2) {
                        thread::sleep(remaining - std::time::Duration::from_millis(1));
                    }
                    while prev.elapsed() < target_frame_time {
                        std::hint::spin_loop();
                    }
                }
            }
            *GLOBAL_LIMIT_CLOCK.write().unwrap() = Some(std::time::Instant::now());
        }
        // end of limiter

        // if the DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING swapchain was created with ALLOW_TEARING, we can present with it.
        // otheriwse it would fail with DXGI_ERROR_INVALID_CALL
        if sync_interval == 0 && handle_opt.is_some() {
            present_flags |= DXGI_PRESENT_ALLOW_TEARING;
        }
        let present_started = std::time::Instant::now();
        let original_present = ORIGINAL_PRESENT.unwrap();
        let mut hr = original_present(p_this, sync_interval, present_flags, p_present_parameters);
        if hr.is_err() && (present_flags.0 & DXGI_PRESENT_ALLOW_TEARING.0) != 0 {
            debug_print!("Present failed with tearing flag: {:#X}, retrying fallback", hr.0);
            let fallback_flags = DXGI_PRESENT(present_flags.0 & !DXGI_PRESENT_ALLOW_TEARING.0);
            hr = original_present(p_this, sync_interval, fallback_flags, p_present_parameters);
            debug_print!("render: Present fallback result={:#X}", hr.0);
        }

        // OBS capture (producer side): run after the original present so the back buffer is
        // stable before the CopyResource — gated internally on READER_ACTIVE.
        capture::capture_on_present(p_this);

        if cfg!(feature = "verbose-logs") {
            let present_ns = present_started.elapsed().as_nanos() as u64;
            // report stalls (maybe it helps some other dev one day)
            if wait_ns > 50_000_000 || present_ns > 50_000_000 {
                debug_print!(
                    "render: STALL thread={} swapchain={:?} wait_ms={:.1} present_ms={:.1} hr={:#X} had_handle={}",
                    GetCurrentThreadId(),
                    p_this,
                    wait_ns as f64 / 1_000_000.0,
                    present_ns as f64 / 1_000_000.0,
                    hr.0,
                    handle_opt.is_some()
                );
            }
            PERF_PRESENTS.set(PERF_PRESENTS.get() + 1);
            PERF_WAIT_NS.set(PERF_WAIT_NS.get() + wait_ns);
            PERF_WAIT_MAX_NS.set(PERF_WAIT_MAX_NS.get().max(wait_ns));
            PERF_PRESENT_NS.set(PERF_PRESENT_NS.get() + present_ns);
            PERF_PRESENT_MAX_NS.set(PERF_PRESENT_MAX_NS.get().max(present_ns));
            let window_start = PERF_WINDOW_START.get().unwrap_or_else(|| {
                let now = std::time::Instant::now();
                PERF_WINDOW_START.set(Some(now));
                now
            });
            if window_start.elapsed() >= std::time::Duration::from_secs(5) {
                let n = PERF_PRESENTS.get().max(1);
                debug_print!(
                    "render: perf thread={} swapchain={:?} presents={} fps={:.0} wait_avg_ms={:.2} wait_max_ms={:.1} present_avg_ms={:.2} present_max_ms={:.1} timeouts={} had_handle={}",
                    GetCurrentThreadId(),
                    p_this,
                    n,
                    n as f64 / window_start.elapsed().as_secs_f64(),
                    PERF_WAIT_NS.get() as f64 / n as f64 / 1_000_000.0,
                    PERF_WAIT_MAX_NS.get() as f64 / 1_000_000.0,
                    PERF_PRESENT_NS.get() as f64 / n as f64 / 1_000_000.0,
                    PERF_PRESENT_MAX_NS.get() as f64 / 1_000_000.0,
                    PERF_TIMEOUTS.get(),
                    handle_opt.is_some()
                );
                PERF_WINDOW_START.set(Some(std::time::Instant::now()));
                PERF_PRESENTS.set(0);
                PERF_WAIT_NS.set(0);
                PERF_WAIT_MAX_NS.set(0);
                PERF_PRESENT_NS.set(0);
                PERF_PRESENT_MAX_NS.set(0);
                PERF_TIMEOUTS.set(0);
            }
        }

        hr
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(_: HINSTANCE, call_reason: u32, _: *mut ()) {
    if call_reason == DLL_PROCESS_ATTACH {
        debug_print!("render: DLL_PROCESS_ATTACH, spawning initialization thread");
        thread::spawn(|| {
            attach();
        });
    } else if call_reason == DLL_PROCESS_DETACH {
        debug_print!("render: DLL_PROCESS_DETACH, cleaning capture state and handles");
        capture::capture_cleanup();
        unsafe {
            let mut guard = WAIT_HANDLE.write().unwrap();
            for (sc, handle) in guard.drain() {
                if !handle.0.is_invalid() {
                    debug_print!("render: closing handle {:?} for swapchain {:#x}", handle.0, sc);
                    let _ = CloseHandle(handle.0);
                }
            }
        }
    }
}
