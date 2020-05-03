use std::rc::Rc;
use tcw3::{
    pal,
    pal::prelude::*,
    ui::{
        layouts::TableLayout,
        theming,
        views::{
            scrollbar::ScrollbarDragListener, Button, Checkbox, Entry, Label, Scrollbar, Spacer,
        },
        AlignFlags,
    },
    uicore::{ActionId, ActionStatus, HView, HWnd, HWndRef, WndListener},
};

struct MyWndListener;

impl WndListener for MyWndListener {
    fn close(&self, wm: pal::Wm, _: HWndRef<'_>) {
        wm.terminate();
    }

    fn interpret_event(
        &self,
        _: pal::Wm,
        _: HWndRef<'_>,
        ctx: &mut tcw3::uicore::InterpretEventCtx<'_>,
    ) {
        ctx.use_accel(&pal::accel_table![
            (
                pal::actions::SELECT_ALL,
                windows("Ctrl+A"),
                gtk("Ctrl+A"),
                macos("Super+A")
            ),
            (
                pal::actions::COPY,
                windows("Ctrl+C"),
                gtk("Ctrl+C"),
                macos("Super+C")
            ),
            (
                pal::actions::CUT,
                windows("Ctrl+X"),
                gtk("Ctrl+X"),
                macos("Super+X")
            ),
            (
                pal::actions::PASTE,
                windows("Ctrl+V"),
                gtk("Ctrl+V"),
                macos("Super+V")
            ),
            (
                pal::actions::UNDO,
                windows("Ctrl+Z"),
                gtk("Ctrl+Z"),
                macos("Super+Z")
            ),
            (
                pal::actions::REDO,
                windows("Ctrl+Y"),
                gtk("Ctrl+Shift+Z"),
                macos("Super+Y")
            ),
            (1, windows("Ctrl+Q"), gtk("Ctrl+Q"), macos("Super+Q")),
            (
                // `SELECT_WORD` is not in the default key bindings of any
                // target platforms
                pal::actions::SELECT_WORD,
                windows("Ctrl+W"),
                gtk("Ctrl+W"),
                macos("Super+W")
            ),
        ]);
    }

    fn validate_action(&self, _: pal::Wm, _: HWndRef<'_>, action: ActionId) -> ActionStatus {
        if action == 1 {
            ActionStatus::VALID | ActionStatus::ENABLED
        } else {
            ActionStatus::empty()
        }
    }

    fn perform_action(&self, wm: pal::Wm, _: HWndRef<'_>, action: ActionId) {
        if action == 1 {
            wm.terminate();
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

    let label = Label::new(style_manager);
    label.set_text("Hello, world! «coi ro do .ui» Saluton! nuqneH");

    let scrollbar = Scrollbar::new(style_manager, false);
    let scrollbar = Rc::new(scrollbar);
    {
        let scrollbar_weak = Rc::downgrade(&scrollbar);
        scrollbar.set_on_drag(move |_| {
            let scrollbar = scrollbar_weak.upgrade().unwrap();
            let value = scrollbar.value();
            Box::new(MyScrollbarDragListener { value, scrollbar })
        });
    }
    {
        let scrollbar_weak = Rc::downgrade(&scrollbar);
        scrollbar.set_on_page_step(move |_, dir| {
            let scrollbar = scrollbar_weak.upgrade().unwrap();
            let value = scrollbar.value() + dir as i8 as f64 * scrollbar.page_step();
            scrollbar.set_value(value.max(0.0).min(1.0));
        });
    }

    let entry = Entry::new(style_manager);

    let button = Button::new(style_manager);
    button.set_caption("Please don't touch this button");

    let checkbox = Checkbox::new(style_manager);
    let checkbox = Rc::new(checkbox);
    checkbox.set_caption("Milk");
    {
        let checkbox_weak = Rc::downgrade(&checkbox);
        checkbox.subscribe_activated(Box::new(move |_| {
            let checkbox = checkbox_weak.upgrade().unwrap();
            checkbox.set_checked(!checkbox.checked());
        }));
    }

    let h_layout = {
        let view = HView::new(Default::default());
        view.set_layout(TableLayout::stack_horz(vec![
            (button.view(), AlignFlags::VERT_JUSTIFY),
            (checkbox.view(), AlignFlags::CENTER),
        ]));
        view
    };

    wnd.content_view().set_layout(
        TableLayout::stack_vert(vec![
            (label.view(), AlignFlags::VERT_JUSTIFY),
            (
                Spacer::new().with_fixed([0.0, 10.0]).into_view(),
                AlignFlags::VERT_JUSTIFY,
            ),
            (scrollbar.view(), AlignFlags::JUSTIFY),
            (
                Spacer::new().with_fixed([0.0, 10.0]).into_view(),
                AlignFlags::VERT_JUSTIFY,
            ),
            (entry.view(), AlignFlags::JUSTIFY),
            (
                Spacer::new().with_fixed([0.0, 10.0]).into_view(),
                AlignFlags::VERT_JUSTIFY,
            ),
            (h_layout.clone(), AlignFlags::JUSTIFY),
        ])
        .with_uniform_margin(20.0),
    );

    wm.enter_main_loop();
}

struct MyScrollbarDragListener {
    scrollbar: Rc<Scrollbar>,
    value: f64,
}

impl ScrollbarDragListener for MyScrollbarDragListener {
    fn motion(&self, _: pal::Wm, new_value: f64) {
        self.scrollbar.set_value(new_value);
    }

    fn cancel(&self, _: pal::Wm) {
        self.scrollbar.set_value(self.value);
    }
}
