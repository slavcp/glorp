use minhook::MinHook;
use std::{
    cell,
    collections::HashMap,
    ffi::c_void,
    mem,
    sync::{LazyLock, RwLock},
    thread,
};
use windows::Win32::{
    Foundation::*,
    Graphics::{
        Direct3D::*,
        Direct3D11::*,
        Dxgi::{Common::*, *},
    },
    System::{Diagnostics::Debug::*, SystemServices::DLL_PROCESS_ATTACH, Threading::*},
};
use windows::core::*;

fn debug_print<T: AsRef<str>>(msg: T) {
    let wide: Vec<u16> = msg.as_ref().encode_utf16().collect();
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

        let device = device.ok_or_else(|| {
            debug_print("D3D11 device creation failed");
            Error::from_win32()
        })?;

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
static mut ORIGINAL_CREATE_SWAPCHAIN: Option<
    unsafe fn(*mut c_void, *mut c_void, *const DXGI_SWAP_CHAIN_DESC1, *mut c_void, *mut *mut c_void) -> HRESULT,
> = None;

#[allow(clippy::type_complexity)]
static mut ORIGINAL_PRESENT: Option<
    unsafe fn(*mut c_void, u32, DXGI_PRESENT, *const DXGI_PRESENT_PARAMETERS) -> HRESULT,
> = None;

static WAIT_HANDLE: LazyLock<RwLock<HashMap<usize, usize>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

fn attach() {
    unsafe {
        let (factory, swap_chain) = get_idxgi().unwrap_or_else(|e| {
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

        let original_present =
            MinHook::create_hook(swap_chain.vtable().Present1 as *mut c_void, present_hk as *mut c_void)
                .unwrap_or_else(|e| {
                    debug_print(format!("d3d11 hook failed: {:?}", e));
                    panic!("hh")
                });

        MinHook::enable_all_hooks().unwrap_or_else(|e| debug_print(format!("cant enable hooks: {:?}", e)));
        #[allow(clippy::missing_transmute_annotations)]
        {
            ORIGINAL_CREATE_SWAPCHAIN = mem::transmute(original_create_swapchain);
            ORIGINAL_PRESENT = mem::transmute(original_present);
        }
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
        desc.BufferCount = 2; // 2 is the minimum
        desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL; // discard crashes
        desc.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
        // desc.Scaling = DXGI_SCALING_NONE; // this crashes
        desc.Flags =
            (DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 | DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0) as u32;

        let original_fn = ORIGINAL_CREATE_SWAPCHAIN.unwrap();

        let result = original_fn(this, pdevice, &desc, prestricttooutput, ppswapchain);
        if let Err(e) = result.ok() {
            debug_print(format!("Failed to create swap chain: {:#X} - {}", result.0, e));
            panic!("h");
        } else {
            let swap_chain = IDXGISwapChain1::from_raw(*ppswapchain);
            if let Ok(swap_chain2) = swap_chain.cast::<IDXGISwapChain2>() {
                swap_chain2
                    .SetMaximumFrameLatency(1)
                    .unwrap_or_else(|e| debug_print(format!("Failed to set latency: {:?}", e)));

                let waitable_obj = swap_chain2.GetFrameLatencyWaitableObject();
                {
                    let mut guard = WAIT_HANDLE.write().unwrap();
                    guard.insert(*ppswapchain as usize, waitable_obj.0 as usize);
                }

                // don't release
                mem::forget(swap_chain2);
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
    thread_local! {
        static INITIALIZED: cell::Cell<bool> = const { cell::Cell::new(false) } ;
    }
    if !INITIALIZED.get() {
        let mut task_index = 0u32;
        unsafe { AvSetMmThreadCharacteristicsW(w!("Games"), &mut task_index) };
        INITIALIZED.set(true);
    }

    unsafe {
        let handle_opt = WAIT_HANDLE.read().unwrap().get(&(p_this as usize)).copied();

        if let Some(h_raw) = handle_opt {
            let h = HANDLE(h_raw as *mut _);
            let _ = WaitForSingleObjectEx(h, u32::MAX, true);
        }

        if sync_interval == 0 {
            present_flags |= DXGI_PRESENT_ALLOW_TEARING;
        }
        let original_present = ORIGINAL_PRESENT.unwrap();
        original_present(p_this, sync_interval, present_flags, p_present_parameters)
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(_: HINSTANCE, call_reason: u32, _: *mut ()) {
    if call_reason == DLL_PROCESS_ATTACH {
        thread::spawn(|| {
            attach();
        });
    }
}
