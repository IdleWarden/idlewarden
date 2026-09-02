// SPDX-License-Identifier: MPL-2.0
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::time::Duration;

use windows::core::{IInspectable, Interface};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::IsWindow;

use crate::frame::pack_rows;
use crate::{CaptureBackend, CaptureError, Frame, Size, WindowHandle};

const BUFFERS: i32 = 2;
const FORMAT: DirectXPixelFormat = DirectXPixelFormat::B8G8R8A8UIntNormalized;
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

/// Windows Graphics Capture of a single window.
pub struct WindowsCapture {
    window: WindowHandle,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    d3d: IDirect3DDevice,
    pool: Direct3D11CaptureFramePool,
    _session: GraphicsCaptureSession,
    _item: GraphicsCaptureItem,
    arrivals: Receiver<()>,
    closed: Arc<AtomicBool>,
    pool_size: SizeInt32,
    next_id: u64,
}

unsafe impl Send for WindowsCapture {}

impl WindowsCapture {
    pub fn new(window: WindowHandle) -> Result<Self, CaptureError> {
        let hwnd = handle(window);
        if !alive(hwnd) {
            return Err(CaptureError::WindowNotFound);
        }

        let (device, context) = create_device()?;
        let d3d = wrap_device(&device)?;

        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(backend)?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(hwnd) }.map_err(|_| {
            if alive(hwnd) {
                CaptureError::ExclusiveFullscreen
            } else {
                CaptureError::WindowNotFound
            }
        })?;

        let pool_size = item.Size().map_err(backend)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&d3d, FORMAT, BUFFERS, pool_size)
            .map_err(backend)?;

        let (sender, arrivals): (SyncSender<()>, Receiver<()>) = sync_channel(1);
        pool.FrameArrived(
            &TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |_, _| {
                let _ = sender.try_send(());
                Ok(())
            }),
        )
        .map_err(backend)?;

        let closed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&closed);
        item.Closed(
            &TypedEventHandler::<GraphicsCaptureItem, IInspectable>::new(move |_, _| {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            }),
        )
        .map_err(backend)?;

        let session = pool.CreateCaptureSession(&item).map_err(backend)?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().map_err(backend)?;

        Ok(WindowsCapture {
            window,
            device,
            context,
            d3d,
            pool,
            _session: session,
            _item: item,
            arrivals,
            closed,
            pool_size,
            next_id: 0,
        })
    }

    fn latest(&self) -> Option<Direct3D11CaptureFrame> {
        let mut newest = None;
        while let Ok(frame) = self.pool.TryGetNextFrame() {
            newest = Some(frame);
        }
        newest
    }

    fn resize_if_needed(&mut self, frame: &Direct3D11CaptureFrame) -> Result<(), CaptureError> {
        let content = frame.ContentSize().map_err(backend)?;
        if content.Width != self.pool_size.Width || content.Height != self.pool_size.Height {
            self.pool
                .Recreate(&self.d3d, FORMAT, BUFFERS, content)
                .map_err(backend)?;
            self.pool_size = content;
        }
        Ok(())
    }

    fn read_back(
        &mut self,
        frame: &Direct3D11CaptureFrame,
    ) -> Result<(Size, Vec<u8>), CaptureError> {
        let surface = frame.Surface().map_err(backend)?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(backend)?;
        let source: ID3D11Texture2D = unsafe { access.GetInterface() }.map_err(backend)?;

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { source.GetDesc(&mut desc) };

        let staging_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            BindFlags: 0,
            MiscFlags: 0,
            ..desc
        };

        let mut staging: Option<ID3D11Texture2D> = None;
        unsafe {
            self.device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging))
        }
        .map_err(backend)?;
        let staging = staging.ok_or_else(|| CaptureError::Backend("no staging texture".into()))?;

        unsafe { self.context.CopyResource(&staging, &source) };

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(backend)?;

        let size = Size {
            width: desc.Width,
            height: desc.Height,
        };
        let pitch = mapped.RowPitch as usize;
        let span = pitch.saturating_mul(size.height as usize);
        let packed = if mapped.pData.is_null() {
            None
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(mapped.pData as *const u8, span) };
            pack_rows(bytes, pitch, size)
        };
        unsafe { self.context.Unmap(&staging, 0) };

        let packed = packed.ok_or_else(|| {
            CaptureError::Backend(format!("unusable mapping: pitch {pitch} for {size:?}"))
        })?;
        Ok((size, packed))
    }

    fn gone(&self) -> CaptureError {
        if alive(handle(self.window)) {
            CaptureError::ExclusiveFullscreen
        } else {
            CaptureError::WindowNotFound
        }
    }
}

impl CaptureBackend for WindowsCapture {
    fn next_frame(&mut self) -> Result<Arc<Frame>, CaptureError> {
        loop {
            if self.closed.load(Ordering::Relaxed) {
                return Err(self.gone());
            }

            if let Some(frame) = self.latest() {
                self.resize_if_needed(&frame)?;
                let captured_at_ms = frame
                    .SystemRelativeTime()
                    .map(|span| (span.Duration / 10_000) as u64)
                    .unwrap_or_default();
                let (size, bgra) = self.read_back(&frame)?;
                self.next_id += 1;
                return Ok(Arc::new(Frame {
                    id: self.next_id,
                    captured_at_ms,
                    size,
                    bgra,
                }));
            }

            match self.arrivals.recv_timeout(FRAME_TIMEOUT) {
                Ok(()) => continue,
                Err(RecvTimeoutError::Timeout) => return Err(self.gone()),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(CaptureError::Backend("the frame pool stopped".into()))
                }
            }
        }
    }

    fn window(&self) -> WindowHandle {
        self.window
    }
}

fn handle(window: WindowHandle) -> HWND {
    HWND(window.0 as *mut std::ffi::c_void)
}

fn alive(hwnd: HWND) -> bool {
    unsafe { IsWindow(Some(hwnd)) }.as_bool()
}

fn create_device() -> Result<(ID3D11Device, ID3D11DeviceContext), CaptureError> {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .map_err(backend)?;

    match (device, context) {
        (Some(device), Some(context)) => Ok((device, context)),
        _ => Err(CaptureError::Backend("D3D11 returned no device".into())),
    }
}

fn wrap_device(device: &ID3D11Device) -> Result<IDirect3DDevice, CaptureError> {
    let dxgi: IDXGIDevice = device.cast().map_err(backend)?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }.map_err(backend)?;
    inspectable.cast().map_err(backend)
}

fn backend(error: windows::core::Error) -> CaptureError {
    CaptureError::Backend(error.message())
}
