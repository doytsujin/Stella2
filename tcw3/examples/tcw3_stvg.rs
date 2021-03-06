use cggeom::prelude::*;
use cgmath::{vec2, Matrix3, Vector2};
use std::cell::RefCell;

use tcw3::{
    pal,
    pal::prelude::*,
    ui::{
        layouts::{EmptyLayout, FillLayout},
        mixins::CanvasMixin,
    },
    uicore::{
        HView, HViewRef, HWnd, HWndRef, SizeTraits, UpdateCtx, ViewFlags, ViewListener, WndListener,
    },
};

static STVG_IMAGE: (&[u8], [f32; 2]) = stvg_macro::include_stvg!("tests/horse.svgz");

struct MyViewListener {
    canvas: RefCell<CanvasMixin>,
}

impl MyViewListener {
    fn new() -> Self {
        Self {
            canvas: RefCell::new(CanvasMixin::new()),
        }
    }
}

impl ViewListener for MyViewListener {
    fn mount(&self, wm: pal::Wm, view: HViewRef<'_>, wnd: HWndRef<'_>) {
        self.canvas.borrow_mut().mount(wm, view, wnd);
    }

    fn unmount(&self, wm: pal::Wm, view: HViewRef<'_>) {
        self.canvas.borrow_mut().unmount(wm, view);
    }

    fn position(&self, wm: pal::Wm, view: HViewRef<'_>) {
        self.canvas.borrow_mut().position(wm, view);
    }

    fn update(&self, wm: pal::Wm, view: HViewRef<'_>, ctx: &mut UpdateCtx<'_>) {
        self.canvas.borrow_mut().update(wm, view, ctx, |draw_ctx| {
            let size = draw_ctx.size;
            let c = &mut draw_ctx.canvas;

            let img_size: Vector2<f32> = STVG_IMAGE.1.into();
            let scale = (size.x / img_size.x).min(size.y / img_size.y);
            let scaled_img_size = img_size * scale;

            use tcw3::stvg::{CanvasStvgExt, Options};

            c.mult_transform(
                Matrix3::from_translation((size - scaled_img_size) * 0.5)
                    * Matrix3::from_scale_2d(scale),
            );
            c.draw_stellavg(STVG_IMAGE.0, &Options::new());
        });
    }
}

struct MyWndListener;

impl WndListener for MyWndListener {
    fn close(&self, wm: pal::Wm, _: HWndRef<'_>) {
        wm.terminate();
    }
}

fn main() {
    if std::env::args().nth(1).map(|x| x == "--export") == Some(true) {
        // Export the StellaVG data. Useful for comparing the compressed size
        // against SVG.
        use std::io::Write;
        std::io::stdout().write_all(&STVG_IMAGE.0).unwrap();
        return;
    }

    let wm = pal::Wm::global();

    println!(
        "The size of the StellaVG image is {} bytes",
        STVG_IMAGE.0.len()
    );

    let wnd = HWnd::new(wm);
    wnd.set_visibility(true);
    wnd.set_listener(MyWndListener);
    wnd.set_caption("demo");

    let v = HView::new(ViewFlags::default());
    v.set_listener(MyViewListener::new());
    v.set_layout(EmptyLayout::new(SizeTraits {
        preferred: vec2(400.0, 500.0),
        ..Default::default()
    }));

    wnd.content_view().set_layout(FillLayout::new(v));

    wm.enter_main_loop();
}
