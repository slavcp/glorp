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
        unsafe {
            std::thread::spawn(|| {
                attach();
            });
        }
    }
}

fn get_factory() -> IDXGIFactory2 {
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

        let mut desc = DXGI_SWAP_CHAIN_DESC1 {
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            Flags: 0,
            ..Default::default()
        };
        desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
        desc.SampleDesc.Count = 1;
        desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
        desc.BufferCount = 2;
        desc.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;
        desc.Flags = 0;

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
            let error_msg = format!("d3d11 create device failed: {:?}", e);
            let wide: Vec<u16> = error_msg.encode_utf16().collect();
            OutputDebugStringW(PCWSTR(wide.as_ptr()));
        }

        let device = device.unwrap();
        let dxgi_device: IDXGIDevice = device.cast().unwrap();
        let dxgi_adapter: IDXGIAdapter = dxgi_device.GetAdapter().unwrap();
        let factory: IDXGIFactory2 = dxgi_adapter.GetParent().unwrap();

        PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)).unwrap();
        factory
    }
}

static mut ORIGINAL_CREATE_SWAPCHAIN: Option<
    unsafe extern "system" fn(
        IDXGIFactory2,
        IUnknown,
        *const DXGI_SWAP_CHAIN_DESC1,
        IDXGIOutput,
    ) -> windows::core::Result<IDXGISwapChain1>,
> = None;

fn attach() {
    unsafe {
        let factory = get_factory();
        let vtable = factory.vtable();

        let hook = MinHook::create_hook(
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

        ORIGINAL_CREATE_SWAPCHAIN = Some(std::mem::transmute::<
            *mut c_void,
            unsafe extern "system" fn(
                IDXGIFactory2,
                IUnknown,
                *const DXGI_SWAP_CHAIN_DESC1,
                IDXGIOutput,
            ) -> windows::core::Result<IDXGISwapChain1>,
        >(hook));
        MinHook::enable_all_hooks().unwrap();
    }
}

unsafe extern "system" fn create_swapchain_hk(
    this: IDXGIFactory2,
    pdevice: IUnknown,
    pdesc: *const DXGI_SWAP_CHAIN_DESC1,
    ppswapchain: IDXGIOutput,
) -> windows::core::Result<IDXGISwapChain1> {
    unsafe {
        let mut modified_desc = *pdesc;
        modified_desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
        modified_desc.BufferCount = 2;
        modified_desc.Flags |= (DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0
            | DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0)
            as u32;
        modified_desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;

        let swap_chain = unsafe {
            ORIGINAL_CREATE_SWAPCHAIN.unwrap()(this, pdevice, &modified_desc, ppswapchain)
        }?;

        if let Ok(swap_chain2) = swap_chain.cast::<IDXGISwapChain2>() {
            swap_chain2.SetMaximumFrameLatency(1).ok();
        }

        Ok(swap_chain)
    }
}
