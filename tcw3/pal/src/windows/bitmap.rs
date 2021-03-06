use arrayvec::ArrayVec;
use cgmath::{Matrix3, Point2};
use std::{convert::TryInto, fmt, mem::MaybeUninit, ptr::null_mut, sync::Arc};
use winapi::{
    shared::minwindef::INT,
    um::{
        gdipluscolor, gdiplusenums,
        gdiplusenums::GraphicsState,
        gdiplusflat as gp,
        gdiplusgpstubs::{
            GpBitmap, GpGraphics, GpMatrix, GpPath, GpPen, GpRect, GpSolidFill, GpStatus,
        },
        gdiplusimaging,
        gdiplusimaging::BitmapData,
        gdiplusinit, gdipluspixelformats,
        gdipluspixelformats::ARGB,
        gdiplustypes,
        gdiplustypes::REAL,
        winnt::CHAR,
    },
};

use super::surface;
use crate::iface;

mod text;

#[cold]
fn panic_by_gp_status(st: GpStatus) -> ! {
    panic!("GDI+ error {:?}", st);
}

/// Panic if `st` is not `Ok`.
fn assert_gp_ok(st: GpStatus) {
    if st != gdiplustypes::Ok {
        panic_by_gp_status(st);
    }
}

unsafe fn create_gp_obj_with<T>(f: impl FnOnce(*mut T) -> GpStatus) -> T {
    let mut out = MaybeUninit::uninit();

    assert_gp_ok(f(out.as_mut_ptr()));

    out.assume_init()
}

/// Call `GdiplusStartup` if it hasn't been called yet.
fn ensure_gdip_inited() {
    lazy_static::lazy_static! {
        static ref GDIP_INIT: () = {
            let input = gdiplusinit::GdiplusStartupInput::new(
                if log::STATIC_MAX_LEVEL == log::LevelFilter::Off {
                    None
                } else {
                    Some(gdip_debug_event_handler)
                },
                0, // do not suppress the GDI+ background thread
                1, // suppress external codecs
            );

            unsafe {
                assert_gp_ok(gdiplusinit::GdiplusStartup(
                    // don't need a token, we won't call `GdiplusShutdown`
                    &mut 0,
                    &input,
                    // output is not necessary because we don't suppress the
                    // GDI+ background thread
                    null_mut(),
                ));
            }
        };
    }

    let () = &*GDIP_INIT;

    extern "system" fn gdip_debug_event_handler(
        level: gdiplusinit::DebugEventLevel,
        message: *mut CHAR,
    ) {
        let level = match level {
            gdiplusinit::DebugEventLevelFatal => log::Level::Error,
            gdiplusinit::DebugEventLevelWarning => log::Level::Warn,
            _ => log::Level::Error,
        };

        log::log!(level, "GDI+ debug event: {:?}", unsafe {
            std::ffi::CStr::from_ptr(message)
        });
    }
}

/// Implements `crate::iface::Bitmap`.
#[derive(Clone)]
pub struct Bitmap {
    pub(super) inner: Arc<BitmapInner>,
}

impl fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bitmap")
            .field("gp_bmp", &self.inner.gp_bmp)
            .field("size", &iface::Bitmap::size(self))
            .finish()
    }
}

impl iface::Bitmap for Bitmap {
    fn size(&self) -> [u32; 2] {
        self.inner.size()
    }
}

/// An owned pointer of `GpBitmap`.
pub(super) struct BitmapInner {
    gp_bmp: *mut GpBitmap,

    /// The compositor representation of the bitmap created in `surface.rs`.
    pub(super) comp_repr: surface::BitmapCompRepr,
}

// I just assume that GDI+ objects only require object-granular external
// synchronization
unsafe impl Send for BitmapInner {}
unsafe impl Sync for BitmapInner {}

impl fmt::Debug for BitmapInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BitmapInner")
            .field("gp_bmp", &self.gp_bmp)
            .finish()
    }
}

impl BitmapInner {
    fn new(size: [u32; 2]) -> Self {
        let gp_bmp = unsafe {
            create_gp_obj_with(|out| {
                gp::GdipCreateBitmapFromScan0(
                    size[0].try_into().expect("bitmap too large"),
                    size[1].try_into().expect("bitmap too large"),
                    0,
                    gdipluspixelformats::PixelFormat32bppPARGB, // pre-multiplied alpha
                    null_mut(),                                 // let GDI+ manage the memory
                    out,
                )
            })
        };

        let comp_repr = surface::BitmapCompRepr::new();

        Self { gp_bmp, comp_repr }
    }

    fn size(&self) -> [u32; 2] {
        let mut out = [0, 0];
        let gp_bmp = self.gp_bmp;
        unsafe {
            assert_gp_ok(gp::GdipGetImageWidth(gp_bmp as _, &mut out[0]));
            assert_gp_ok(gp::GdipGetImageHeight(gp_bmp as _, &mut out[1]));
        }
        [out[0] as u32, out[1] as u32]
    }
}

impl Drop for BitmapInner {
    fn drop(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipDisposeImage(self.gp_bmp as _));
        }
    }
}

pub(super) struct BitmapWriteGuard<'a> {
    bmp: &'a BitmapInner,
    data: BitmapData,
}

pub(super) struct BitmapReadGuard<'a> {
    bmp: &'a BitmapInner,
    data: BitmapData,
}

impl BitmapInner {
    fn lock(&self, flags: u32) -> BitmapData {
        let size = self.size();
        unsafe {
            let mut out = MaybeUninit::uninit();
            assert_gp_ok(gp::GdipBitmapLockBits(
                self.gp_bmp,
                &GpRect {
                    X: 0,
                    Y: 0,
                    Width: size[0] as i32,
                    Height: size[1] as i32,
                },
                flags,
                gdipluspixelformats::PixelFormat32bppPARGB,
                out.as_mut_ptr(),
            ));
            out.assume_init()
        }
    }

    pub(super) fn read(&self) -> BitmapReadGuard<'_> {
        let data = self.lock(gdiplusimaging::ImageLockModeRead);

        BitmapReadGuard { bmp: self, data }
    }

    fn write(&self) -> BitmapWriteGuard<'_> {
        let data = self.lock(gdiplusimaging::ImageLockModeWrite);

        BitmapWriteGuard { bmp: self, data }
    }
}

impl BitmapWriteGuard<'_> {
    fn size(&self) -> [u32; 2] {
        [self.data.Width, self.data.Height]
    }

    fn stride(&self) -> u32 {
        self.data.Stride.abs() as u32
    }

    fn as_ptr(&self) -> *mut u8 {
        self.data.Scan0 as _
    }
}

impl Drop for BitmapWriteGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipBitmapUnlockBits(self.bmp.gp_bmp, &mut self.data));
        }
    }
}

impl BitmapReadGuard<'_> {
    pub fn size(&self) -> [u32; 2] {
        [self.data.Width, self.data.Height]
    }

    pub fn stride(&self) -> u32 {
        self.data.Stride.abs() as u32
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.data.Scan0 as _
    }
}

impl Drop for BitmapReadGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipBitmapUnlockBits(self.bmp.gp_bmp, &mut self.data));
        }
    }
}

/// An owned pointer of `GpGraphics`.
#[derive(Debug)]
struct UniqueGpGraphics {
    gp_gr: *mut GpGraphics,
}

impl Drop for UniqueGpGraphics {
    fn drop(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipDeleteGraphics(self.gp_gr));
        }
    }
}

/// An owned pointer of `GpPath`.
#[derive(Debug)]
struct UniqueGpPath {
    gp_path: *mut GpPath,
}

impl Drop for UniqueGpPath {
    fn drop(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipDeletePath(self.gp_path));
        }
    }
}

/// An owned pointer of `GpSolidFill`.
#[derive(Debug)]
struct UniqueGpSolidFill {
    gp_solid_fill: *mut GpSolidFill,
}

impl Drop for UniqueGpSolidFill {
    fn drop(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipDeleteBrush(self.gp_solid_fill as _));
        }
    }
}

/// An owned pointer of `GpPen`.
#[derive(Debug)]
struct UniqueGpPen {
    gp_pen: *mut GpPen,
}

impl Drop for UniqueGpPen {
    fn drop(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipDeletePen(self.gp_pen));
        }
    }
}

/// An owned pointer of `GpMatrix`.
#[derive(Debug)]
struct UniqueGpMatrix {
    gp_mat: *mut GpMatrix,
}

impl Drop for UniqueGpMatrix {
    fn drop(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipDeleteMatrix(self.gp_mat));
        }
    }
}

fn rgbaf32_to_argb(c: iface::RGBAF32) -> ARGB {
    use alt_fp::FloatOrd;
    let cvt = |x: f32| (x.fmin(1.0).fmax(0.0) * 255.0) as u8;

    let c = c.map_rgb(cvt).map_alpha(cvt);
    gdipluscolor::Color::MakeARGB(c.a, c.r, c.g, c.b)
}

/// Implements `crate::iface::BitmapBuilder`.
#[derive(Debug)]
pub struct BitmapBuilder {
    bmp: BitmapInner,
    gr: UniqueGpGraphics,
    path: UniqueGpPath,
    brush: UniqueGpSolidFill,
    brush2: UniqueGpSolidFill,
    pen: UniqueGpPen,
    mat: UniqueGpMatrix,
    state_stack: ArrayVec<[GraphicsState; 16]>,
    cur_pt: [REAL; 2],
}

impl iface::BitmapBuilderNew for BitmapBuilder {
    fn new(size: [u32; 2]) -> Self {
        ensure_gdip_inited();

        let bmp = BitmapInner::new(size);

        let gr = UniqueGpGraphics {
            gp_gr: unsafe {
                create_gp_obj_with(|out| gp::GdipGetImageGraphicsContext(bmp.gp_bmp as _, out))
            },
        };

        unsafe {
            gp::GdipSetSmoothingMode(gr.gp_gr, gdiplusenums::SmoothingModeAntiAlias);
            gp::GdipTranslateWorldTransform(gr.gp_gr, -0.5, -0.5, gdiplusenums::MatrixOrderPrepend);
        }

        let path = UniqueGpPath {
            gp_path: unsafe {
                create_gp_obj_with(|out| gp::GdipCreatePath(gdiplusenums::FillModeWinding, out))
            },
        };

        let brush = UniqueGpSolidFill {
            gp_solid_fill: unsafe {
                create_gp_obj_with(|out| gp::GdipCreateSolidFill(0xffffffff, out))
            },
        };
        let brush2 = UniqueGpSolidFill {
            gp_solid_fill: unsafe {
                create_gp_obj_with(|out| gp::GdipCreateSolidFill(0xffffffff, out))
            },
        };

        let pen = UniqueGpPen {
            gp_pen: unsafe {
                create_gp_obj_with(|out| {
                    gp::GdipCreatePen1(0xff000000, 1.0, gdiplusenums::UnitPixel, out)
                })
            },
        };

        let mat = UniqueGpMatrix {
            gp_mat: unsafe { create_gp_obj_with(|out| gp::GdipCreateMatrix(out)) },
        };

        Self {
            bmp,
            gr,
            path,
            brush,
            brush2,
            pen,
            mat,
            state_stack: ArrayVec::new(),
            cur_pt: [0.0; 2],
        }
    }
}

impl iface::BitmapBuilder for BitmapBuilder {
    type Bitmap = Bitmap;

    fn into_bitmap(self) -> Self::Bitmap {
        Bitmap {
            inner: Arc::new(self.bmp),
        }
    }
}

impl iface::Canvas for BitmapBuilder {
    fn save(&mut self) {
        let st = unsafe { create_gp_obj_with(|out| gp::GdipSaveGraphics(self.gr.gp_gr, out)) };
        self.state_stack.push(st);
    }
    fn restore(&mut self) {
        let st = self.state_stack.pop().unwrap();
        unsafe {
            assert_gp_ok(gp::GdipRestoreGraphics(self.gr.gp_gr, st));
        }
    }
    fn begin_path(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipResetPath(self.path.gp_path));
            assert_gp_ok(gp::GdipSetPathFillMode(
                self.path.gp_path,
                gdiplusenums::FillModeWinding,
            ));
        }
    }
    fn close_path(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipClosePathFigure(self.path.gp_path));
        }
    }
    fn move_to(&mut self, p: Point2<f32>) {
        unsafe {
            assert_gp_ok(gp::GdipStartPathFigure(self.path.gp_path));
        }
        self.cur_pt = p.into();
    }
    fn line_to(&mut self, p: Point2<f32>) {
        unsafe {
            assert_gp_ok(gp::GdipAddPathLine(
                self.path.gp_path,
                self.cur_pt[0],
                self.cur_pt[1],
                p.x,
                p.y,
            ));
        }
        self.cur_pt = p.into();
    }
    fn cubic_bezier_to(&mut self, cp1: Point2<f32>, cp2: Point2<f32>, p: Point2<f32>) {
        unsafe {
            assert_gp_ok(gp::GdipAddPathBezier(
                self.path.gp_path,
                self.cur_pt[0],
                self.cur_pt[1],
                cp1.x,
                cp1.y,
                cp2.x,
                cp2.y,
                p.x,
                p.y,
            ));
        }
        self.cur_pt = p.into();
    }
    fn quad_bezier_to(&mut self, cp: Point2<f32>, p: Point2<f32>) {
        let p1: Point2<f32> = self.cur_pt.into();

        let cp1 = cp + (p1 - cp) * (1.0 / 3.0);
        let cp2 = cp + (p - cp) * (1.0 / 3.0);

        self.cubic_bezier_to(cp1, cp2, p);
    }
    fn fill(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipFillPath(
                self.gr.gp_gr,
                self.brush.gp_solid_fill as _,
                self.path.gp_path,
            ));
        }
        self.begin_path();
    }
    fn stroke(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipDrawPath(
                self.gr.gp_gr,
                self.pen.gp_pen,
                self.path.gp_path,
            ));
        }
        self.begin_path();
    }
    fn clip(&mut self) {
        unsafe {
            assert_gp_ok(gp::GdipSetClipPath(
                self.gr.gp_gr,
                self.path.gp_path,
                gdiplusenums::CombineModeIntersect,
            ));
        }
        self.begin_path();
    }
    fn set_fill_rgb(&mut self, rgb: iface::RGBAF32) {
        unsafe {
            assert_gp_ok(gp::GdipSetSolidFillColor(
                self.brush.gp_solid_fill,
                rgbaf32_to_argb(rgb),
            ));
        }
    }
    fn set_stroke_rgb(&mut self, rgb: iface::RGBAF32) {
        unsafe {
            assert_gp_ok(gp::GdipSetPenColor(self.pen.gp_pen, rgbaf32_to_argb(rgb)));
        }
    }
    fn set_line_cap(&mut self, cap: iface::LineCap) {
        let cap = match cap {
            iface::LineCap::Butt => gdiplusenums::LineCapFlat,
            iface::LineCap::Round => gdiplusenums::LineCapRound,
            iface::LineCap::Square => gdiplusenums::LineCapSquare,
        };

        unsafe {
            assert_gp_ok(gp::GdipSetPenEndCap(self.pen.gp_pen, cap));
        }
    }
    fn set_line_join(&mut self, join: iface::LineJoin) {
        let join = match join {
            iface::LineJoin::Miter => gdiplusenums::LineJoinMiter,
            iface::LineJoin::Bevel => gdiplusenums::LineJoinBevel,
            iface::LineJoin::Round => gdiplusenums::LineJoinRound,
        };

        unsafe {
            assert_gp_ok(gp::GdipSetPenLineJoin(self.pen.gp_pen, join));
        }
    }
    fn set_line_dash(&mut self, phase: f32, lengths: &[f32]) {
        unsafe {
            if lengths.len() == 0 {
                assert_gp_ok(gp::GdipSetPenDashStyle(
                    self.pen.gp_pen,
                    gdiplusenums::DashStyleSolid,
                ));
            } else {
                assert_gp_ok(gp::GdipSetPenDashArray(
                    self.pen.gp_pen,
                    lengths.as_ptr(),
                    lengths.len() as INT,
                ));
            }
            assert_gp_ok(gp::GdipSetPenDashOffset(self.pen.gp_pen, phase));
        }
    }
    fn set_line_width(&mut self, width: f32) {
        unsafe {
            assert_gp_ok(gp::GdipSetPenWidth(self.pen.gp_pen, width));
        }
    }
    fn set_line_miter_limit(&mut self, miter_limit: f32) {
        unsafe {
            assert_gp_ok(gp::GdipSetPenMiterLimit(self.pen.gp_pen, miter_limit));
        }
    }
    fn mult_transform(&mut self, m: Matrix3<f32>) {
        let m = m / m.z.z;

        unsafe {
            assert_gp_ok(gp::GdipSetMatrixElements(
                self.mat.gp_mat,
                m.x.x,
                m.x.y,
                m.y.x,
                m.y.y,
                m.z.x,
                m.z.y,
            ));
            assert_gp_ok(gp::GdipMultiplyWorldTransform(
                self.gr.gp_gr,
                self.mat.gp_mat,
                gdiplusenums::MatrixOrderPrepend,
            ));
        }
    }
}

/// Create a monochrome noise image.
pub fn new_noise_bmp() -> Bitmap {
    struct Xorshift32(u32);

    impl Xorshift32 {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            self.0
        }
    }

    const SIZE: usize = 128;

    ensure_gdip_inited();

    let bmp = Bitmap {
        inner: Arc::new(BitmapInner::new([SIZE as u32; 2])),
    };

    {
        let bmp_data = bmp.inner.write();
        debug_assert_eq!(bmp_data.size(), [SIZE as u32; 2]);
        assert!(bmp_data.stride() >= SIZE as u32 * 4);
        assert!(bmp_data.stride() % 4 == 0);
        let data = unsafe {
            std::slice::from_raw_parts_mut(
                bmp_data.as_ptr(),
                (SIZE - 1) * bmp_data.stride() as usize + SIZE * 4,
            )
        };

        let mut rng = Xorshift32(0x4F6CDD1D);
        for pix in data.chunks_exact_mut(4) {
            let rnd = rng.next().to_ne_bytes();

            // Approximate Gaussian distribution
            let color = (rnd[0] as u32 + rnd[1] as u32 + rnd[2] as u32 + rnd[3] as u32) / 4;

            // Tone adjustment
            let color = (color * color / 256) as u8;

            pix[0] = color;
            pix[1] = color;
            pix[2] = color;
            pix[3] = 0xff;
        }
    }

    bmp
}
