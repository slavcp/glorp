use minhook::MinHook;
use std::ffi::c_void;
use windows::Win32::{
    Foundation::*,
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
fn debug_print(msg: &str) {
    let wide: Vec<u16> = msg.encode_utf16().collect();
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
            D3D11_CREATE_DEVICE_DEBUG,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        ) {
            debug_print(format!("d3d11 create device failed: {:?}", e).as_str());
        }

        let device = device.ok_or_else(|| {
            debug_print("D3D11 device creation failed");
            Error::from_win32()
        })?;

        let dxgi_device: IDXGIDevice = device.cast().map_err(|e| {
            debug_print(format!("Failed to cast device to IDXGIDevice: {:?}", e).as_str());
            e
        })?;

        let dxgi_adapter: IDXGIAdapter = dxgi_device.GetAdapter().map_err(|e| {
            let error_msg = format!("Failed to get adapter: {:?}", e);
            let wide: Vec<u16> = error_msg.encode_utf16().collect();
            OutputDebugStringW(PCWSTR(wide.as_ptr()));
            e
        })?;

        let factory: IDXGIFactory2 = dxgi_adapter.GetParent().map_err(|e| {
            let error_msg = format!("Failed to get factory: {:?}", e);
            let wide: Vec<u16> = error_msg.encode_utf16().collect();
            OutputDebugStringW(PCWSTR(wide.as_ptr()));
            e
        })?;

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
        let factory = match get_factory() {
            Ok(f) => f,
            Err(e) => {
                let error_msg = format!("Failed to get factory: {:?}", e);
                let wide: Vec<u16> = error_msg.encode_utf16().collect();
                OutputDebugStringW(PCWSTR(wide.as_ptr()));
                panic!("Failed to get factory");
            }
        };

        let vtable = factory.vtable();

        let original_fn = MinHook::create_hook(
            vtable.CreateSwapChainForComposition as *mut c_void,
            create_swapchain_hk as *mut c_void,
        )
        .unwrap_or_else(|e| {
            let error_msg = format!("d3d11 hook failed: {:?}", e);
            let wide: Vec<u16> = error_msg.encode_utf16().collect();
            OutputDebugStringW(PCWSTR(wide.as_ptr()));
            panic!("hh")
        });

        let error_msg = format!("factory: {:?}", factory);
        let wide: Vec<u16> = error_msg.encode_utf16().collect();
        OutputDebugStringW(PCWSTR(wide.as_ptr()));
        MinHook::enable_all_hooks().unwrap();

        ORIGINAL_CREATE_SWAPCHAIN = Some(std::mem::transmute(original_fn));
    }
}

unsafe extern "system" fn create_swapchain_hk(
    this: *mut c_void,
    pdevice: *mut c_void,
    _pdesc: *const DXGI_SWAP_CHAIN_DESC1,
    prestricttooutput: *mut c_void,
    ppswapchain: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: 0,
            Height: 0,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            Stereo: windows::Win32::Foundation::BOOL(1),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 1,
            Scaling: DXGI_SCALING_NONE,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: (DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0
                | DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0) as u32,
        };

        let original_fn = match ORIGINAL_CREATE_SWAPCHAIN {
            Some(f) => f,
            None => {
                debug_print("Original function is None");
                return HRESULT(0);
            }
        };

        let result = original_fn(this, pdevice, &desc, prestricttooutput, ppswapchain);

        //cast from the default IDXGISwapChain1 to IDXGISwapChain2
        if let Some(swap_chain) = (*ppswapchain as *mut IDXGISwapChain2).as_mut() {
            swap_chain.SetMaximumFrameLatency(1).ok();
        }

        debug_print("SUCCESS!");

        result
    }
}
