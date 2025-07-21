use minhook::MinHook;
use std::ffi::c_void;
use windows::Win32::{
    Foundation::{HINSTANCE, HMODULE, LPARAM, WPARAM},
    Graphics::{
        Direct3D::*,
        Direct3D11::*,
        Dxgi::{Common::*, *},
    },
    System::{Diagnostics::Debug::*, SystemServices::*},
    UI::WindowsAndMessaging::*,
};
use windows::core::*;

#[unsafe(no_mangle)]
extern "system" fn DllMain(_: HINSTANCE, call_reason: u32, _: *mut ()) {
    if call_reason == DLL_PROCESS_ATTACH {
        std::thread::spawn(|| {
            attach();
        });
    }
}

fn debug_print<T: AsRef<str>>(msg: T) {
    let wide: Vec<u16> = msg.as_ref().encode_utf16().collect();
    unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())) };
}

fn get_idxgi() -> Result<(IDXGIFactory2, IDXGISwapChain1)> {
    unsafe {
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("nf"),
            WINDOW_STYLE(0),
            0,
            0,
            1,
            1,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let mut device: Option<ID3D11Device> = None;

        if let Err(e) = D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_SINGLETHREADED,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        ) {
            debug_print(format!("d3d11 create device failed: {:?}", e));
        }

        let device = device.ok_or_else(|| {
            debug_print("D3D11 device creation failed");
            Error::from_win32()
        })?;

        let dxgi_device: IDXGIDevice = device.cast().unwrap_or_else(|e| {
            debug_print(format!("Failed to cast device to IDXGIDevice: {:?}", e));
            panic!("Failed to get device");
        });

        let dxgi_adapter: IDXGIAdapter = dxgi_device.GetAdapter().unwrap_or_else(|e| {
            debug_print(format!("Failed to get adapter: {:?}", e));
            panic!("Failed to get adapter");
        });

        let factory: IDXGIFactory2 = dxgi_adapter.GetParent().unwrap_or_else(|e| {
            debug_print(format!("Failed to get factory: {:?}", e));
            panic!("Failed to get factory");
        });

        let swap_chain: IDXGISwapChain1 = factory
            .CreateSwapChainForComposition(
                &dxgi_device,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: 1,
                    Height: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    Stereo: BOOL(0),
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    Scaling: DXGI_SCALING_STRETCH,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                    AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                    Flags: 0,
                },
                None,
            )
            .unwrap_or_else(|e| {
                debug_print(format!("Failed to create swapchain: {:?}", e));
                panic!("Failed to create swapchain");
            });

        let present1_fn = swap_chain.vtable().Present1 as *const c_void;
        debug_print(format!("Present1 function pointer: {:?}", present1_fn));

        PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)).unwrap();
        Ok((factory, swap_chain))
    }
}

#[allow(clippy::type_complexity)]
static mut ORIGINAL_CREATE_SWAPCHAIN: Option<
    unsafe fn(
        *mut c_void,
        *mut c_void,
        *const DXGI_SWAP_CHAIN_DESC1,
        *mut c_void,
        *mut *mut c_void,
    ) -> HRESULT,
> = None;
// static mut ORIGINAL_PRESENT: unsafe fn(
//     *mut c_void,
//     u32,
//     DXGI_PRESENT,
//     *const DXGI_PRESENT_PARAMETERS,
// ) -> HRESULT = dummy_present_hk;

// unsafe fn dummy_present_hk(
//     _: *mut c_void,
//     _: u32,
//     _: DXGI_PRESENT,
//     _: *const DXGI_PRESENT_PARAMETERS,
// ) -> HRESULT {
//     panic!("ORIGINAL_PRESENT called before initialization");
// }

fn attach() {
    unsafe {
        let (factory, _swap_chain) = get_idxgi().unwrap_or_else(|e| {
            debug_print(format!("Failed to get factory and swap chain: {:?}", e));
            panic!("Failed to get factory and swap chain");
        });

        let original_create_swapchain = MinHook::create_hook(
            factory.vtable().CreateSwapChainForComposition as *mut c_void,
            create_swapchain_hk as *mut c_void,
        )
        .unwrap_or_else(|e| {
            debug_print(format!("d3d11 hook failed: {:?}", e));
            panic!("hh")
        });

        // let original_present = MinHook::create_hook(
        //     swap_chain.vtable().Present1 as *mut c_void,
        //     present_hk as *mut c_void,
        // )
        // .unwrap_or_else(|e| {
        //     debug_print(format!("d3d11 hook failed: {:?}", e));
        //     panic!("hh")
        // });

        debug_print(format!("factory: {:?}", factory));

        MinHook::enable_all_hooks().unwrap();
        ORIGINAL_CREATE_SWAPCHAIN = std::mem::transmute::<
            *mut c_void,
            Option<
                unsafe fn(
                    *mut c_void,
                    *mut c_void,
                    *const DXGI_SWAP_CHAIN_DESC1,
                    *mut c_void,
                    *mut *mut c_void,
                ) -> HRESULT,
            >,
        >(original_create_swapchain);
        // ORIGINAL_PRESENT = std::mem::transmute(original_present);
    }
}

// In create_swapchain_hk, remove the unwrap()
unsafe extern "system" fn create_swapchain_hk(
    this: *mut c_void,
    pdevice: *mut c_void,
    pdesc: *const DXGI_SWAP_CHAIN_DESC1,
    prestricttooutput: *mut c_void,
    ppswapchain: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        let mut desc = *pdesc;
        desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
        // 2 is lowest allowed for DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL
        desc.BufferCount = 2;
        desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
        desc.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
        desc.Flags = (DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0
            | DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0) as u32;

        let original_fn = ORIGINAL_CREATE_SWAPCHAIN.unwrap();

        let result = original_fn(this, pdevice, &desc, prestricttooutput, ppswapchain);

        if result.is_ok() {
            let swap_chain = IDXGISwapChain1::from_raw(*ppswapchain);
            if let Ok(swap_chain2) = swap_chain.cast::<IDXGISwapChain2>() {
                if let Err(e) = swap_chain2.SetMaximumFrameLatency(1) {
                    debug_print(format!("Failed to set latency: {:?}", e));
                }
                // not releasing the swapchain leads to breaking the present hook
                std::mem::forget(swap_chain2);
            }
        } else {
            debug_print("Failed to create swap chain");
        }
        //  HRESULT(0) is success
        debug_print(format!("result: {:?}", result));
        result
    }
}

// static mut RENDER_FPS: f64 = 0.0;
// static mut FRAME_COUNT: f64 = 0.0;
// static mut LAST_FPS_UPDATE: Option<Instant> = None;
// static FRAME_TIME_NS: u64 = 16 * 1000 * 1000;
// static TIME_OF_LAST_PRESENT_NS: AtomicU64 = AtomicU64::new(0);

// unsafe extern "system" fn present_hk(
//     p_this: *mut c_void,
//     sync_interval: u32,
//     present_flags: DXGI_PRESENT,
//     p_present_parameters: *const DXGI_PRESENT_PARAMETERS,
// ) -> HRESULT {
//     unsafe {
//         FRAME_COUNT += 1.0;
//         let now = Instant::now();
//         if let None = LAST_FPS_UPDATE {
//             LAST_FPS_UPDATE = Some(now);
//         }
//         if now.duration_since(LAST_FPS_UPDATE.unwrap()).as_secs_f64() >= 0.5 {
//             RENDER_FPS = FRAME_COUNT * 2.0;
//             FRAME_COUNT = 0.0;
//             LAST_FPS_UPDATE = Some(now);
//             debug_print(format!("Render FPS: {}", unsafe { RENDER_FPS }));
//         }

//         let frame_time_ns = FRAME_TIME_NS;
//         if frame_time_ns != 0 {
//             let current_time_ns = SystemTime::now()
//                 .duration_since(SystemTime::UNIX_EPOCH)
//                 .unwrap()
//                 .as_nanos() as u64;
//             let time_between_last_present_call =
//                 current_time_ns - TIME_OF_LAST_PRESENT_NS.load(Ordering::Relaxed);

//             if time_between_last_present_call < frame_time_ns {
//                 let sleep_duration =
//                     Duration::from_nanos(frame_time_ns - time_between_last_present_call);

//                 if sleep_duration > Duration::from_millis(1) {
//                     std::thread::sleep(sleep_duration - Duration::from_millis(1));
//                 }

//                 let spin_start = Instant::now();
//                 while spin_start.elapsed() < Duration::from_millis(1) {
//                     std::hint::spin_loop();
//                 }
//             }

//             TIME_OF_LAST_PRESENT_NS.store(
//                 SystemTime::now()
//                     .duration_since(SystemTime::UNIX_EPOCH)
//                     .unwrap()
//                     .as_nanos() as u64,
//                 Ordering::Relaxed,
//             );
//         }

//         ORIGINAL_PRESENT(p_this, sync_interval, present_flags, p_present_parameters)
//     }
// }
