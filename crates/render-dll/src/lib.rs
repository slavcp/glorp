use minhook::MinHook;
use std::ffi::c_void;
use windows::Win32::{
    Foundation::{BOOL, HINSTANCE, HMODULE, LPARAM, WPARAM},
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
    match call_reason {
        DLL_PROCESS_ATTACH => {
            std::thread::spawn(|| {
                attach();
            });
        }
        DLL_PROCESS_DETACH => unsafe {
            MinHook::disable_all_hooks().ok();
            MinHook::uninitialize();
        },
        _ => (),
    }
}
fn debug_print<T: AsRef<str>>(msg: T) {
    let wide: Vec<u16> = msg.as_ref().encode_utf16().collect();
    unsafe { OutputDebugStringW(PCWSTR(wide.as_ptr())) };
}

fn get_factory() -> Result<IDXGIFactory2> {
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

        PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)).unwrap();
        Ok(factory)
    }
}

static mut ORIGINAL_CREATE_SWAPCHAIN: Option<
    unsafe fn(
        *mut c_void,
        *mut c_void,
        *const DXGI_SWAP_CHAIN_DESC1,
        *mut c_void,
        *mut *mut c_void,
    ) -> HRESULT,
> = None;

fn attach() {
    unsafe {
        let factory = get_factory().unwrap_or_else(|e| {
            debug_print(format!("Failed to get factory: {:?}", e));
            panic!("Failed to get factory");
        });

        let vtable = factory.vtable();

        let original_fn = MinHook::create_hook(
            vtable.CreateSwapChainForComposition as *mut c_void,
            create_swapchain_hk as *mut c_void,
        )
        .unwrap_or_else(|e| {
            debug_print(format!("d3d11 hook failed: {:?}", e));
            panic!("hh")
        });

        debug_print(format!("factory: {:?}", factory));

        MinHook::enable_all_hooks().unwrap();
        ORIGINAL_CREATE_SWAPCHAIN = Some(std::mem::transmute(original_fn));
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
        let mut desc = *pdesc;
        desc.Stereo = BOOL(0);
        desc.SampleDesc = DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        };

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
            } else {
                debug_print("Failed to cast to IDXGISwapChain2");
            }
        } else {
            debug_print("Failed to create swap chain");
        }
        result
    }
}
