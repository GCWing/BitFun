//! Windows.Graphics.Capture (WGC) single-frame window capture.
//!
//! Used as tier-2 fallback when `PrintWindow` returns an all-black bitmap for
//! DirectComposition / UWP / WinUI3 surfaces. Requires Windows 10 1903+.

#![allow(dead_code)]

use bitfun_core::util::errors::{BitFunError, BitFunResult};
use std::time::{Duration, Instant};
use windows::core::Interface;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

/// Capture one frame from `hwnd` via WGC, returning top-down BGRA bytes.
pub(super) fn capture_window_bgra(hwnd: HWND) -> BitFunResult<(Vec<u8>, u32, u32)> {
    if hwnd.is_invalid() {
        return Err(BitFunError::service(
            "WGC capture: invalid HWND".to_string(),
        ));
    }

    unsafe {
        // Best-effort COM init for WinRT factory calls from a worker thread.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let (d3d_device, d3d_context) = create_d3d11_device()?;
        let direct_device = create_winrt_d3d_device(&d3d_device)?;

        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(|e| BitFunError::service(format!("WGC factory: {e}")))?;
        let item: GraphicsCaptureItem = interop
            .CreateForWindow(hwnd)
            .map_err(|e| BitFunError::service(format!("WGC CreateForWindow: {e}")))?;

        let size = item
            .Size()
            .map_err(|e| BitFunError::service(format!("WGC item Size: {e}")))?;
        if size.Width <= 0 || size.Height <= 0 {
            return Err(BitFunError::service(format!(
                "WGC capture: invalid item size {}x{}",
                size.Width, size.Height
            )));
        }

        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &direct_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .map_err(|e| BitFunError::service(format!("WGC CreateFreeThreaded: {e}")))?;

        let session = frame_pool
            .CreateCaptureSession(&item)
            .map_err(|e| BitFunError::service(format!("WGC CreateCaptureSession: {e}")))?;
        session
            .StartCapture()
            .map_err(|e| BitFunError::service(format!("WGC StartCapture: {e}")))?;

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut last_err: Option<String>;
        let result = loop {
            match frame_pool.TryGetNextFrame() {
                Ok(frame) => match copy_frame_to_bgra(&frame, &d3d_device, &d3d_context) {
                    Ok(pixels) => break Ok(pixels),
                    Err(e) => last_err = Some(e.to_string()),
                },
                Err(e) => last_err = Some(format!("TryGetNextFrame: {e}")),
            }
            if Instant::now() >= deadline {
                break Err(BitFunError::service(format!(
                    "WGC capture timed out waiting for frame{}",
                    last_err
                        .map(|e| format!(" (last error: {e})"))
                        .unwrap_or_default()
                )));
            }
            std::thread::sleep(Duration::from_millis(25));
        };

        let _ = session.Close();
        let _ = frame_pool.Close();

        result
    }
}

unsafe fn create_d3d11_device() -> BitFunResult<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;

    // SAFETY: output pointers reference live `Option` slots, and all remaining
    // arguments are documented D3D11 constants or null/default handles.
    if unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            flags,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .is_err()
    {
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                flags,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .map_err(|e| BitFunError::service(format!("D3D11CreateDevice (WARP): {e}")))?;
    }

    let device = device.ok_or_else(|| {
        BitFunError::service("D3D11CreateDevice returned null device".to_string())
    })?;
    let context = context.ok_or_else(|| {
        BitFunError::service("D3D11CreateDevice returned null context".to_string())
    })?;
    Ok((device, context))
}

unsafe fn create_winrt_d3d_device(d3d_device: &ID3D11Device) -> BitFunResult<IDirect3DDevice> {
    let dxgi_device: IDXGIDevice = d3d_device
        .cast()
        .map_err(|e| BitFunError::service(format!("IDXGIDevice cast: {e}")))?;
    // SAFETY: `dxgi_device` is a live COM interface obtained from the supplied
    // D3D11 device and remains alive through the conversion call.
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
        .map_err(|e| BitFunError::service(format!("CreateDirect3D11DeviceFromDXGIDevice: {e}")))?;
    inspectable
        .cast()
        .map_err(|e| BitFunError::service(format!("IDirect3DDevice cast: {e}")))
}

unsafe fn copy_frame_to_bgra(
    frame: &windows::Graphics::Capture::Direct3D11CaptureFrame,
    d3d_device: &ID3D11Device,
    d3d_context: &ID3D11DeviceContext,
) -> BitFunResult<(Vec<u8>, u32, u32)> {
    let surface = frame
        .Surface()
        .map_err(|e| BitFunError::service(format!("WGC frame Surface: {e}")))?;
    let access: IDirect3DDxgiInterfaceAccess = surface
        .cast()
        .map_err(|e| BitFunError::service(format!("IDirect3DDxgiInterfaceAccess cast: {e}")))?;
    // SAFETY: `access` is the live DXGI interface for `surface`; the requested
    // interface type matches the WGC frame surface contract.
    let src_texture: ID3D11Texture2D = unsafe { access.GetInterface::<ID3D11Texture2D>() }
        .map_err(|e| BitFunError::service(format!("GetInterface ID3D11Texture2D: {e}")))?;

    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { src_texture.GetDesc(&mut desc) };
    let width = desc.Width;
    let height = desc.Height;
    if width == 0 || height == 0 {
        return Err(BitFunError::service(
            "WGC frame texture has zero dimensions".to_string(),
        ));
    }

    let staging_desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: desc.SampleDesc,
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut staging: Option<ID3D11Texture2D> = None;
    // SAFETY: `staging_desc` is fully initialized and `staging` is a live
    // output slot for the newly created texture interface.
    unsafe { d3d_device.CreateTexture2D(&staging_desc, None, Some(&mut staging)) }
        .map_err(|e| BitFunError::service(format!("CreateTexture2D staging: {e}")))?;
    let staging = staging.ok_or_else(|| {
        BitFunError::service("CreateTexture2D returned null staging texture".to_string())
    })?;

    unsafe { d3d_context.CopyResource(&staging, &src_texture) };

    let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { d3d_context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
        .map_err(|e| BitFunError::service(format!("Map staging texture: {e}")))?;

    let row_pitch = mapped.RowPitch as usize;
    let width_bytes = (width as usize) * 4;
    let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
    let src = mapped.pData as *const u8;
    // SAFETY: a successful `Map` exposes `height` rows at `pData`, each with
    // at least `width * 4` readable bytes according to `RowPitch`. Destination
    // rows are disjoint slices of the fully allocated `pixels` buffer.
    for y in 0..height as usize {
        let src_row = unsafe { src.add(y * row_pitch) };
        let dst_row = unsafe { pixels.as_mut_ptr().add(y * width_bytes) };
        unsafe { std::ptr::copy_nonoverlapping(src_row, dst_row, width_bytes) };
    }

    unsafe { d3d_context.Unmap(&staging, 0) };

    Ok((pixels, width, height))
}

const D3D11_SDK_VERSION: u32 = 7;
