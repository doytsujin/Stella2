use tcw3::{pal, pal::prelude::*, uicore};

fn main() {
    let wm = pal::wm();

    pal::invoke_on_main_thread(|_| {
        // The following statement panics if we are not on the main thread
        pal::wm();
    });

    let wnd = uicore::HWnd::new(wm);
    wnd.set_visibility(true);

    wm.enter_main_loop();
}
