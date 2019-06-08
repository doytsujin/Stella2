use bitflags::bitflags;
use cggeom::{box2, prelude::*, Box2};
use cgmath::{Matrix3, Point2, Rad, Vector2};
use std::rc::Rc;

use crate::pal::{SysFontType, RGBAF32};

bitflags! {
    /// A set of styling classes.
    pub struct ClassSet: u8 {
        /// The mouse pointer inside the element.
        const HOVER = 1 << 0;
        /// The element is active, e.g., a button is being pressed down.
        const ACTIVE = 1 << 1;
        /// The element is a button's border.
        const BUTTON = 1 << 2;
    }
}

/// `ClassSet` of an element and its ancestors.
#[derive(Debug, Clone)]
pub struct ElemClassPath {
    pub tail: Option<Rc<ElemClassPath>>,
    pub class_set: ClassSet,
}

impl ElemClassPath {
    pub fn new(class_set: ClassSet, tail: Option<Rc<ElemClassPath>>) -> Self {
        Self { tail, class_set }
    }
}

impl Default for ElemClassPath {
    fn default() -> Self {
        Self {
            tail: None,
            class_set: ClassSet::empty(),
        }
    }
}

/// A role of a subview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Generic,
}

/// Represents a single styling property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prop {
    /// The number of layers.
    NumLayers,

    /// The [`HImg`] of the `n`-th layer.
    ///
    /// [`HImg`]: crate::ui::images::HImg
    LayerImg(u32),

    /// The background color ([`RGBAF32`]) of the `n`-th layer.
    ///
    /// [`RGBAF32`]: crate::pal::RGBAF32
    LayerBgColor(u32),

    /// The [`Metrics`] of the `n`-th layer.
    LayerMetrics(u32),

    /// The opacity of the `n`-th layer.
    LayerOpacity(u32),

    /// The `content_center` of the `n`-th layer.
    LayerCenter(u32),

    /// The transformation of the `n`-th layer.
    LayerXform(u32),

    /// The [`Metrics`] of a subview.
    SubviewMetrics(Role),

    /// The [`Metrics`] of the layer used to clip subviews.
    ClipMetrics,

    /// The minimum size.
    MinSize,

    /// The default foreground color.
    FgColor,

    /// The default `SysFontType`.
    Font,
}

#[derive(Debug, Clone)]
pub enum PropValue {
    Float(f32),
    Usize(usize),
    Himg(Option<crate::ui::images::HImg>),
    Rgbaf32(RGBAF32),
    Metrics(Metrics),
    Vector2(Vector2<f32>),
    Point2(Point2<f32>),
    Box2(Box2<f32>),
    LayerXform(LayerXform),
    SysFontType(SysFontType),
}

impl PropValue {
    pub fn default_for_prop(prop: &Prop) -> Self {
        match prop {
            Prop::NumLayers => PropValue::Usize(0),
            Prop::LayerImg(_) => PropValue::Himg(None),
            Prop::LayerBgColor(_) => PropValue::Rgbaf32(RGBAF32::new(0.0, 0.0, 0.0, 0.0)),
            Prop::LayerMetrics(_) => PropValue::Metrics(Metrics::default()),
            Prop::LayerOpacity(_) => PropValue::Float(1.0),
            Prop::LayerCenter(_) => PropValue::Box2(box2! {
                min: [0.0, 0.0], max: [1.0, 1.0]
            }),
            Prop::LayerXform(_) => PropValue::LayerXform(LayerXform::default()),
            Prop::SubviewMetrics(_) => PropValue::Metrics(Metrics::default()),
            Prop::ClipMetrics => PropValue::Metrics(Metrics::default()),
            Prop::MinSize => PropValue::Vector2(Vector2::new(0.0, 0.0)),
            Prop::FgColor => PropValue::Rgbaf32(RGBAF32::new(0.0, 0.0, 0.0, 1.0)),
            Prop::Font => PropValue::SysFontType(SysFontType::Normal),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    /// Distances from corresponding edges. Non-finite values (e.g., NaN) mean
    /// flexible space. Edges are specified in the clock-wise order, starting
    /// from top.
    pub margin: [f32; 4],
    /// The size of a layer. Non-finite values (e.g., NaN) mean the size is
    /// unspecified.
    pub size: Vector2<f32>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            margin: [0.0; 4],
            size: [std::f32::NAN; 2].into(),
        }
    }
}

impl Metrics {
    pub(crate) fn arrange(&self, container: Box2<f32>, default_size: Vector2<f32>) -> Box2<f32> {
        let mut kid_size = self.size;
        if !kid_size.x.is_finite() {
            kid_size.x = default_size.x;
        }
        if !kid_size.y.is_finite() {
            kid_size.y = default_size.y;
        }

        let mut frame = container;

        let margin = self.margin;
        if margin[3].is_finite() {
            frame.min.x += margin[3];
        }
        if margin[1].is_finite() {
            frame.max.x -= margin[1];
        }
        match (margin[3].is_finite(), margin[1].is_finite()) {
            (false, false) => {
                let mid = (frame.min.x + frame.max.x) * 0.5;
                frame.min.x = mid - kid_size.x * 0.5;
                frame.max.x = mid + kid_size.x * 0.5;
            }
            (true, false) => frame.max.x = frame.min.x + kid_size.x,
            (false, true) => frame.min.x = frame.max.x - kid_size.x,
            (true, true) => {}
        }

        if margin[0].is_finite() {
            frame.min.y += margin[0];
        }
        if margin[2].is_finite() {
            frame.max.y -= margin[2];
        }
        match (margin[0].is_finite(), margin[2].is_finite()) {
            (false, false) => {
                let mid = (frame.min.y + frame.max.y) * 0.5;
                frame.min.y = mid - kid_size.y * 0.5;
                frame.max.y = mid + kid_size.y * 0.5;
            }
            (true, false) => frame.max.y = frame.min.y + kid_size.y,
            (false, true) => frame.min.y = frame.max.y - kid_size.y,
            (true, true) => {}
        }

        frame
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LayerXform {
    pub anchor: Point2<f32>,
    pub scale: [f32; 2],
    pub rotate: Rad<f32>,
    pub translate: Vector2<f32>,
}

impl Default for LayerXform {
    fn default() -> Self {
        Self {
            anchor: [0.5; 2].into(),
            scale: [1.0; 2],
            rotate: Rad(0.0),
            translate: [0.0; 2].into(),
        }
    }
}

impl LayerXform {
    pub fn to_matrix3(&self, bounds: Box2<f32>) -> Matrix3<f32> {
        let anchor = Vector2::new(
            bounds.min.x + bounds.size().x * self.anchor.x,
            bounds.min.y + bounds.size().y * self.anchor.y,
        );
        Matrix3::from_translation(self.translate + anchor)
            * Matrix3::from_angle(self.rotate)
            * Matrix3::from_nonuniform_scale_2d(self.scale[0], self.scale[1])
            * Matrix3::from_translation(-anchor)
    }
}
