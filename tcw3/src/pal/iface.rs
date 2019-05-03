//! Defines an abstract interface to the backend implementation.
//!
//! This module defines an abstract interface not bound to any specific types
//! defined in the backend implementation.
//!
//! The parent module (`pal`) provides type aliases for the types defined here,
//! specialized for the default backend, as well as simple re-exports of
//! non-generic types.
use bitflags::bitflags;
use cggeom::Box2;
use cgmath::{Matrix3, Point2};
use rgb::RGBA;
use std::{fmt::Debug, rc::Rc};

pub type RGBAF32 = RGBA<f32>;

pub trait WM: Sized + Debug + 'static {
    /// A window handle type.
    type HWnd: Debug + Send + Sync + Clone;

    /// A layer handle type.
    type HLayer: Debug + Send + Sync + Clone;

    /// A bitmap type.
    type Bitmap: Bitmap;

    /// Get the default instance of [`WM`]. It only can be called by a main thread.
    fn global() -> &'static Self;

    /// Get the default instance of [`WM`] without checking the calling thread.
    unsafe fn global_unchecked() -> &'static Self;

    /// Enqueue a call to the specified function on the main thread. The calling
    /// thread can be any thread.
    fn invoke_on_main_thread(f: impl FnOnce(&'static Self) + Send + 'static);

    /// Enqueue a call to the specified function on the main thread.
    fn invoke(&self, f: impl FnOnce(&'static Self) + 'static);

    fn enter_main_loop(&self);
    fn terminate(&self);

    fn new_wnd(&self, attrs: &WndAttrs<Self, &str, Self::HLayer>) -> Self::HWnd;

    /// Set the attributes of a window.
    ///
    /// Panics if the window has already been closed.
    fn set_wnd_attr(&self, window: &Self::HWnd, attrs: &WndAttrs<Self, &str, Self::HLayer>);
    fn remove_wnd(&self, window: &Self::HWnd);
    /// Update a window's contents.
    ///
    /// Calling this method requests that the contents of a window is updated
    /// based on the attributes of the window and its all sublayers as soon as
    /// possible. Conversely, all attribute updates may be deferred until this
    /// method is called.
    fn update_wnd(&self, window: &Self::HWnd);
    /// Get the size of a window's content region.
    fn get_wnd_size(&self, window: &Self::HWnd) -> [u32; 2];

    fn new_layer(&self, attrs: &LayerAttrs<Self::Bitmap, Self::HLayer>) -> Self::HLayer;

    // FIXME: Maybe pass `LayerAttrs` by value to elide the costly copy?
    /// Set the attributes of a layer.
    ///
    /// The behavior is unspecified if the layer has already been removed.
    fn set_layer_attr(&self, layer: &Self::HLayer, attrs: &LayerAttrs<Self::Bitmap, Self::HLayer>);
    fn remove_layer(&self, layer: &Self::HLayer);
}

#[derive(Clone)]
pub struct WndAttrs<T: WM, TCaption, TLayer> {
    /// The size of the content region.
    pub size: Option<[u32; 2]>,
    pub min_size: Option<[u32; 2]>,
    pub max_size: Option<[u32; 2]>,
    pub flags: Option<WndFlags>,
    pub caption: Option<TCaption>,
    pub visible: Option<bool>,
    pub listener: Option<Option<Rc<dyn WndListener<T>>>>,
    pub layer: Option<Option<TLayer>>,
}

impl<T: WM, TCaption, TLayer> Default for WndAttrs<T, TCaption, TLayer> {
    fn default() -> Self {
        Self {
            size: None,
            min_size: None,
            max_size: None,
            flags: None,
            caption: None,
            visible: None,
            listener: None,
            layer: None,
        }
    }
}

impl<T: WM, TCaption, TLayer> WndAttrs<T, TCaption, TLayer>
where
    TCaption: AsRef<str>,
    TLayer: Clone,
{
    pub fn as_ref(&self) -> WndAttrs<T, &str, TLayer> {
        WndAttrs {
            size: self.size,
            min_size: self.min_size,
            max_size: self.max_size,
            flags: self.flags,
            caption: self.caption.as_ref().map(AsRef::as_ref),
            visible: self.visible,
            listener: self.listener.clone(),
            layer: self.layer.clone(),
        }
    }
}

bitflags! {
    pub struct WndFlags: u32 {
        const RESIZABLE = 1 << 0;
        const BORDERLESS = 1 << 1;
    }
}

impl Default for WndFlags {
    fn default() -> Self {
        WndFlags::RESIZABLE
    }
}

#[derive(Debug, Clone)]
pub struct LayerAttrs<TBitmap, TLayer> {
    /// The 2D transformation applied to the contents of the layer.
    /// It doesn't have an effect on sublayers.
    ///
    /// The input coordinate space is based on `bounds`. The output coordinate
    /// space is virtual pixel coordinates with `(0,0)` at the top left corner
    /// of a window's client region.
    ///
    /// `value.x.z` and `value.y.z` may be assumed to be zero. This means
    /// projective transformations are not supported and only affine
    /// transformations can be expressed. `value.z.z` must be positive.
    pub transform: Option<Matrix3<f32>>,

    /// Specifies the content image of the layer.
    pub contents: Option<Option<TBitmap>>,
    /// Specifies the bounds of the content image.
    ///
    /// Because of how the anchor point is calculated in the macOS bakcend, it
    /// must not be empty.
    pub bounds: Option<Box2<f32>>,
    /// Specifies the flexible region of the content image.
    ///
    /// Defaults to `(0,0)-(1,1)`, indicating entire the image is scaled in
    /// both directions to match the content bounds.
    pub contents_center: Option<Box2<f32>>,
    /// Specifies the natural scaling ratio of the content image.
    ///
    /// This is used only when `contents_center` has a non-default value.
    /// Defaults to `1.0`.
    pub contents_scale: Option<f32>,
    /// Specifies the solid color underlaid to the content image.
    pub bg_color: Option<RGBAF32>,

    pub sublayers: Option<Vec<TLayer>>,

    /// Specifies the opacity value.
    ///
    /// Defaults to `1.0`. Sublayers are affected as well. The opacity value
    /// is applied after the sublayers are composited, thus it has a different
    /// effect than applying the value on the sublayers individually.
    pub opacity: Option<f32>,

    /// Specifies additional options on the layer.
    pub flags: Option<LayerFlags>,
}

impl<TBitmap, TLayer> LayerAttrs<TBitmap, TLayer> {
    /// Replace the fields with values from `o` if they are `Some(_)`.
    pub fn override_with(&mut self, o: Self) {
        macro_rules! process_one {
            ($i:ident) => {
                if let Some(x) = o.$i {
                    self.$i = Some(x);
                }
            };
        }
        process_one!(transform);
        process_one!(contents);
        process_one!(bounds);
        process_one!(contents_center);
        process_one!(contents_scale);
        process_one!(bg_color);
        process_one!(sublayers);
        process_one!(opacity);
        process_one!(flags);
    }
}

impl<TBitmap, TLayer> Default for LayerAttrs<TBitmap, TLayer> {
    fn default() -> Self {
        Self {
            transform: None,
            contents: None,
            bounds: None,
            contents_center: None,
            contents_scale: None,
            sublayers: None,
            bg_color: None,
            opacity: None,
            flags: None,
        }
    }
}

bitflags! {
    pub struct LayerFlags: u32 {
        /// Clip sublayers to the content bounds.
        const MASK_TO_BOUNDS = 1;
    }
}

/// Window event handlers.
///
/// The receiver is immutable because event handlers may manipulate windows,
/// which in turn might cause other event handlers to be called.
pub trait WndListener<T: WM> {
    /// The user has attempted to close a window. Returns `true` if the window
    /// can be closed.
    fn close_requested(&self, _: &T, _: &T::HWnd) -> bool {
        true
    }

    /// A window has been closed.
    fn close(&self, _: &T, _: &T::HWnd) {}

    /// A window is being resized.
    ///
    /// While the user is resizing a window, this method is called repeatedly
    /// as the window's outline is dragged.
    ///
    /// The new window size can be retrieved using [`WM::get_wnd_size`].
    /// Based on the new window size, The client (the implementer of this trait)
    /// should relayout, update composition layers, and call [`WM::update_wnd`]
    /// in this method.
    fn resize(&self, _: &T, _: &T::HWnd) {}

    // TODO: more events
}

/// A immutable, ref-counted bitmap image.
pub trait Bitmap: Clone + Sized + Send + Sync + Debug {}

/// Types supporting drawing operations.
pub trait Canvas: Debug {
    /// Push a copy of the current graphics state onto the state stack.
    fn save(&mut self);
    /// Pop a graphics state from the state stack.
    fn restore(&mut self);

    /// Start a new empty path.
    fn begin_path(&mut self);
    /// Close the current figure of the current path.
    fn close_path(&mut self);

    /// Begin a new subpath at the specified point.
    fn move_to(&mut self, p: Point2<f32>);
    /// Append a straight line to the specified point.
    fn line_to(&mut self, p: Point2<f32>);
    /// Append a cubic Bézier curve to the specified point, using the provided
    /// control points.
    fn cubic_bezier_to(&mut self, cp1: Point2<f32>, cp2: Point2<f32>, p: Point2<f32>);
    /// Append a quadratic Bézier curve to the specified point, using the
    /// provided control point.
    fn quad_bezier_to(&mut self, cp: Point2<f32>, p: Point2<f32>);

    /// Fill the area within the current path, using the non-zero winding number
    /// rule.
    fn fill(&mut self);
    /// Draw a line along the current path.
    fn stroke(&mut self);
    /// Set the current clipping region to its intersection with the area within
    /// current path.
    fn clip(&mut self);

    /// Stroke the specified rectangle.
    ///
    /// The implementation of this method may invalidate the current path.
    fn stroke_rect(&mut self, bx: Box2<f32>) {
        self.begin_path();
        self.move_to(Point2::new(bx.min.x, bx.min.y));
        self.line_to(Point2::new(bx.max.x, bx.min.y));
        self.line_to(Point2::new(bx.max.x, bx.max.y));
        self.line_to(Point2::new(bx.min.x, bx.max.y));
        self.close_path();
        self.stroke();
    }
    /// Fill the specified rectangle.
    ///
    /// The implementation of this method may invalidate the current path.
    fn fill_rect(&mut self, bx: Box2<f32>) {
        self.begin_path();
        self.move_to(Point2::new(bx.min.x, bx.min.y));
        self.line_to(Point2::new(bx.max.x, bx.min.y));
        self.line_to(Point2::new(bx.max.x, bx.max.y));
        self.line_to(Point2::new(bx.min.x, bx.max.y));
        self.close_path();
        self.fill();
    }
    /// Set the current clipping region to its intersection with the specified
    /// rectangle.
    ///
    /// The implementation of this method may invalidate the current path.
    fn clip_rect(&mut self, bx: Box2<f32>) {
        self.begin_path();
        self.move_to(Point2::new(bx.min.x, bx.min.y));
        self.line_to(Point2::new(bx.max.x, bx.min.y));
        self.line_to(Point2::new(bx.max.x, bx.max.y));
        self.line_to(Point2::new(bx.min.x, bx.max.y));
        self.close_path();
        self.clip();
    }

    /// Set the current fill brush to a solid color.
    fn set_fill_rgb(&mut self, rgb: RGBAF32);
    // TODO: generic brush

    /// Set the current stroke brush to a solid color.
    fn set_stroke_rgb(&mut self, rgb: RGBAF32);
    // TODO: generic brush

    fn set_line_cap(&mut self, cap: LineCap);
    fn set_line_join(&mut self, join: LineJoin);
    fn set_line_dash(&mut self, phase: f32, lengths: &[f32]);
    fn set_line_width(&mut self, width: f32);
    fn set_line_miter_limit(&mut self, miter_limit: f32);

    /// Transform the local coordinate system.
    ///
    /// `m.x.z` and `m.y.z` is assumed to be zero. This means projective
    /// transformations are not supported and only affine transformations can
    /// be expressed. `m.z.z` must be positive.
    fn mult_transform(&mut self, m: Matrix3<f32>);

    // TODO: text rendering

    // TODO: image rendering
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// A builder type for [`Bitmap`] supporting 2D drawing operations via
/// [`Canvas`].
pub trait BitmapBuilder: Canvas {
    type Bitmap: Bitmap;

    fn into_bitmap(self) -> Self::Bitmap;
}

pub trait BitmapBuilderNew: BitmapBuilder + Sized {
    /// Create a [`BitmapBuilder`] with a R8G8B8A8 backing bitmap.
    fn new(size: [u32; 2]) -> Self;
}