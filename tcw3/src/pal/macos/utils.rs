// Based on <https://github.com/rust-windowing/winit/blob/master/src/platform/macos/window.rs>
use cocoa::{
    base::{id, nil},
    foundation::NSAutoreleasePool,
};
use objc::{msg_send, sel, sel_impl, runtime::{Object, Sel, YES, BOOL}};
use std::ops::Deref;

pub struct IdRef(id);

impl IdRef {
    pub fn new(i: id) -> IdRef {
        IdRef(i)
    }

    #[allow(dead_code)]
    pub fn retain(i: id) -> IdRef {
        if i != nil {
            let _: id = unsafe { msg_send![i, retain] };
        }
        IdRef(i)
    }

    pub fn non_nil(self) -> Option<IdRef> {
        if self.0 == nil {
            None
        } else {
            Some(self)
        }
    }
}

impl Drop for IdRef {
    fn drop(&mut self) {
        if self.0 != nil {
            with_autorelease_pool(|| unsafe {
                let _: () = msg_send![self.0, release];
            });
        }
    }
}

impl Deref for IdRef {
    type Target = id;
    fn deref<'a>(&'a self) -> &'a id {
        &self.0
    }
}

impl Clone for IdRef {
    fn clone(&self) -> IdRef {
        if self.0 != nil {
            let _: id = unsafe { msg_send![self.0, retain] };
        }
        IdRef(self.0)
    }
}

extern "C" fn yes(_: &Object, _: Sel) -> BOOL {
    YES
}

pub fn with_autorelease_pool(f: impl FnOnce()) {
    unsafe {
        let autoreleasepool = NSAutoreleasePool::new(nil);
        f();
        let _: () = msg_send![autoreleasepool, release];
    }
}
