use neo_linked_list::{linked_list::Node, AssertUnpin, LinkedListCell};
use std::pin::Pin;

use crate::pal::{prelude::*, MtSticky, Wm};

#[allow(clippy::type_complexity)]
static ON_UPDATE_DISPATCHES: MtSticky<LinkedListCell<AssertUnpin<dyn FnOnce(Wm)>>> = Init::INIT;

/// Implements `WmExt::invoke_on_update`.
pub fn invoke_on_update(wm: Wm, f: impl FnOnce(Wm) + 'static) {
    invoke_on_update_inner(wm, Node::pin(AssertUnpin::new(f)));
}

fn invoke_on_update_inner(wm: Wm, f: Pin<Box<Node<AssertUnpin<dyn FnOnce(Wm)>>>>) {
    let queue = ON_UPDATE_DISPATCHES.get_with_wm(wm);
    if queue.is_empty() {
        wm.invoke(process_pending_invocations);
    }
    queue.push_back_node(f);
}

/// Process pending invocations.
pub fn process_pending_invocations(wm: Wm) {
    loop {
        let f = ON_UPDATE_DISPATCHES.get_with_wm(wm).pop_front_node();
        if let Some(f) = f {
            blackbox(move || {
                (Pin::into_inner(f).element.inner)(wm);
            });
        } else {
            break;
        }
    }
}

/// Limits the stack usage of repeated calls to an unsized closure.
/// (See The Rust Unstable Book, `unsized_locals` for more.)
#[inline(never)]
pub(super) fn blackbox<R>(f: impl FnOnce() -> R) -> R {
    f()
}
