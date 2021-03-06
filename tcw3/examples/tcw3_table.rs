use std::ops::Range;
use tcw3::{
    pal,
    pal::prelude::*,
    ui::{
        layouts::FillLayout,
        prelude::*,
        theming,
        views::{table, table::LineTy, Label, ScrollableTable},
    },
    uicore::{HView, HWnd, HWndRef, SizeTraits, WndListener},
};

struct MyWndListener;

impl WndListener for MyWndListener {
    fn close(&self, wm: pal::Wm, _: HWndRef<'_>) {
        wm.terminate();
    }
}

struct TableModelQuery {
    style_manager: &'static theming::Manager,
}

impl table::TableModelQuery for TableModelQuery {
    fn new_view(&mut self, cell: table::CellIdx) -> (HView, Box<dyn table::CellCtrler>) {
        let label = Label::new(self.style_manager);
        label.set_text(format!("{:?}", cell));

        (label.view(), Box::new(()))
    }

    fn range_size(&mut self, line_ty: LineTy, range: Range<u64>, _approx: bool) -> f64 {
        (range.end - range.start) as f64
            * match line_ty {
                LineTy::Row => 20.0,
                LineTy::Col => 200.0,
            }
    }
}

fn main() {
    env_logger::init();

    let wm = pal::Wm::global();
    let style_manager = theming::Manager::global(wm);

    let wnd = HWnd::new(wm);
    wnd.set_visibility(true);
    wnd.set_listener(MyWndListener);

    let table = ScrollableTable::new(style_manager);

    table.table().set_size_traits(SizeTraits {
        preferred: [200.0, 300.0].into(),
        ..Default::default()
    });

    // Set up the table model
    {
        let mut edit = table.table().edit().unwrap();
        edit.set_model(TableModelQuery { style_manager });
        edit.insert(LineTy::Row, 0..500_000_000_000_000);
        edit.insert(LineTy::Col, 0..300);
        edit.set_scroll_pos([0.0, edit.scroll_pos()[1]]);
    }

    wnd.content_view()
        .set_layout(FillLayout::new(table.view()).with_uniform_margin(10.0));

    wm.enter_main_loop();
}
