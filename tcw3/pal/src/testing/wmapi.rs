use cgmath::Point2;
use std::time::Instant;

use crate::{iface, HWnd};

/// Provides access to a virtual environment.
///
/// This is provided as a trait so that testing code can be compiled even
/// without a `testing` feature flag.
pub trait TestingWm: 'static {
    /// Get the global instance of [`tcw3::pal::Wm`]. This is identical to
    /// calling `Wm::global()`.
    ///
    /// [`tcw3::pal::Wm`]: crate::Wm
    fn wm(&self) -> crate::Wm;

    /// Process events until all `!Send` dispatches (those generated by
    /// `Wm::invoke`, but not `Wm::invoke_on_main_thread`) are processed.
    fn step_unsend(&self);

    /// Process events until at least one event is processed.
    fn step(&self);

    /// Process events until at least one event is processed or
    /// until the specified instant.
    fn step_until(&self, till: Instant);

    /// Get a list of currently open windows.
    fn hwnds(&self) -> Vec<HWnd>;

    /// Get the attributes of a window.
    fn wnd_attrs(&self, hwnd: &HWnd) -> Option<WndAttrs>;

    /// Trigger `WndListener::close_requested`.
    fn raise_close_requested(&self, hwnd: &HWnd);

    /// Set a given window's DPI scale and trigger
    /// `WndListener::dpi_scale_changed`.
    ///
    /// `dpi_scale` must be positive and finite.
    ///
    /// TODO: Add a method to set the default DPI scale
    fn set_wnd_dpi_scale(&self, hwnd: &HWnd, dpi_scale: f32);

    /// Set a given window's size and trigger `WndListener::resize`.
    ///
    /// `size` is not automatically clipped by `min_size` or `max_size`.
    fn set_wnd_size(&self, hwnd: &HWnd, size: [u32; 2]);

    /// Trigger `WndListener::mouse_motion`.
    fn raise_mouse_motion(&self, hwnd: &HWnd, loc: Point2<f32>);

    /// Trigger `WndListener::mouse_leave`.
    fn raise_mouse_leave(&self, hwnd: &HWnd);

    /// Trigger `WndListener::mouse_drag`.
    fn raise_mouse_drag(&self, hwnd: &HWnd, loc: Point2<f32>, button: u8) -> Box<dyn MouseDrag>;
}

/// A snapshot of window attributes.
#[derive(Debug, Clone)]
pub struct WndAttrs {
    pub size: [u32; 2],
    pub min_size: [u32; 2],
    pub max_size: [u32; 2],
    pub flags: iface::WndFlags,
    pub caption: String,
    pub visible: bool,
}

/// Provides an interface for simulating a mouse drag geature.
///
/// See [`MouseDragListener`] for the semantics of the methods.
///
/// [`MouseDragListener`]: crate::iface::MouseDragListener
pub trait MouseDrag {
    /// Trigger `MouseDragListener::mouse_motion`.
    fn mouse_motion(&self, _loc: Point2<f32>);
    /// Trigger `MouseDragListener::mouse_down`.
    fn mouse_down(&self, _loc: Point2<f32>, _button: u8);
    /// Trigger `MouseDragListener::mouse_up`.
    fn mouse_up(&self, _loc: Point2<f32>, _button: u8);
    /// Trigger `MouseDragListener::cancel`.
    fn cancel(&self);
}
