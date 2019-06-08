use iterpool::{intrusive_list, Pool, PoolPtr};
use quick_error2::quick_error;
use std::{cell::RefCell, fmt, sync::Arc};

use crate::{
    pal::{iface::WM as _, Bitmap, MtLock, MtSticky, WM},
    uicore::HWnd,
};

/// A bitmap created by rasterizing [`Img`]. The second value represents the
/// actual DPI scale value of the bitmap, which may or may not match the
/// `dpi_scale` passed to `Img::new_bmp`.
pub type Bmp = (Bitmap, f32);

/// An implementation of an image with an abstract representation.
pub trait Img: Send + Sync + 'static {
    /// Construct a `Bitmap` for the specified DPI scale.
    ///
    /// Returns a constructed `Bitmap` and the actual DPI scale of the `Bitmap`.
    fn new_bmp(&self, dpi_scale: f32) -> Bmp;
}

/// Represents an image with an abstract representation.
#[derive(Debug, Clone)]
pub struct HImg {
    inner: Arc<ImgInner<dyn Img>>,
}

struct ImgInner<T: ?Sized> {
    cache_ref: MtSticky<RefCell<ImgCacheRef>>,
    img: T,
}

#[derive(Debug)]
struct ImgCacheRef {
    /// A pointer to a `CacheImg` in `Cache::imgs`.
    img_ptr: Option<PoolPtr>,
}

impl HImg {
    pub fn new(img: impl Img) -> Self {
        Self {
            inner: Arc::new(ImgInner {
                cache_ref: MtSticky::new(RefCell::new(ImgCacheRef { img_ptr: None })),
                img,
            }),
        }
    }

    /// Construct a `Bitmap` for the specified DPI scale. Uses a global cache,
    /// which is owned by the main thread (hence the `WM` parameter).
    ///
    /// The cache only stores `Bmp`s created for DPI scale values used by any of
    /// open windows. For other DPI scale values, this method behaves like
    /// `new_bmp_uncached`.
    ///
    /// Returns a constructed `Bitmap` and the actual DPI scale of the `Bitmap`.
    pub fn new_bmp(&self, wm: WM, dpi_scale: f32) -> Bmp {
        let mut cache_ref = self
            .inner
            .cache_ref
            .get_with_wm(wm)
            .try_borrow_mut()
            .expect("can't call `new_bmp` recursively on the same image");

        let dpi_scale = DpiScale::new(dpi_scale).unwrap();

        let mut cache = CACHE.get_with_wm(wm).borrow_mut();

        let img_ptr = *cache_ref.img_ptr.get_or_insert_with(|| {
            // `CacheImg` isn't in the cache, create one
            cache.img_add()
        });

        // Try the cache
        if let Some(bmp) = cache.img_find_bmp(img_ptr, dpi_scale) {
            return bmp.clone();
        }

        // Not in the cache. Create a brand new `Bmp`.
        //
        // Unborrow the cache temporarily so that `Img::new_bmp` can
        // recursively call `new_bmp` for other images.
        drop(cache);

        let bmp = self.inner.img.new_bmp(dpi_scale.value());

        // Find the `CacheDpiScale` object.
        let mut cache = CACHE.get_with_wm(wm).borrow_mut();
        let dpi_scale_ptr = if let Some(x) = cache.dpi_scale_find(dpi_scale) {
            x
        } else {
            // Unrecognized DPI scale, cache is unavailable
            return bmp;
        };

        // Insert the `Bmp` to the cache.
        cache.img_add_bmp(img_ptr, dpi_scale_ptr, bmp.clone());

        bmp
    }

    /// Construct a `Bitmap` for the specified DPI scale. Does not use a cache
    /// and always calls [`Img::new_bmp`] directly.
    ///
    /// Returns a constructed `Bitmap` and the actual DPI scale of the `Bitmap`.
    pub fn new_bmp_uncached(&self, dpi_scale: f32) -> Bmp {
        self.inner.img.new_bmp(dpi_scale)
    }
}

impl Drop for ImgCacheRef {
    fn drop(&mut self) {
        if let Some(img_ptr) = self.img_ptr {
            // `ImgCacheRef` is wrapped by `MtSticky`, so `WM::global()` will succeed
            let wm = WM::global();
            CACHE.get_with_wm(wm).borrow_mut().img_remove(img_ptr);
        }
    }
}

impl<T: ?Sized> fmt::Debug for ImgInner<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ImgInner")
            .field("cache_ref", &self.cache_ref)
            .field("img", &((&self.img) as *const _))
            .finish()
    }
}

/// This function is called by `uicore` to update the list of known DPI scale
/// values based on open windows.
///
/// This isn't a good practice from a modular design point of view since
/// it creates a circular dependency between `ui` and `uicore`. That being said,
/// I think it's better justified than ending up with boilerplate code in the
/// application.
pub(crate) fn handle_new_wnd(hwnd: &HWnd) {
    use std::cell::Cell;

    struct ListenerState {
        wm: WM,
        dpi_scale: Cell<DpiScale>,
    }

    impl Drop for ListenerState {
        fn drop(&mut self) {
            // This method is called when the window is destroyed.
            // Use `invoke` because we don't know the state of the call stack
            // when `drop` is called.
            let dpi_scale = self.dpi_scale.get();
            self.wm.invoke(move |wm| {
                CACHE
                    .get_with_wm(wm)
                    .borrow_mut()
                    .dpi_scale_release(dpi_scale);
            });
        }
    }

    let state = ListenerState {
        wm: hwnd.wm(),
        dpi_scale: Cell::new(DpiScale::new(hwnd.dpi_scale()).unwrap()),
    };
    CACHE
        .get_with_wm(hwnd.wm())
        .borrow_mut()
        .dpi_scale_add_ref(state.dpi_scale.get());

    hwnd.subscribe_dpi_scale_changed(Box::new(move |wm, hwnd| {
        let state = &state;
        let new_dpi_scale = DpiScale::new(hwnd.dpi_scale()).unwrap();
        if new_dpi_scale != state.dpi_scale.get() {
            let mut cache = CACHE.get_with_wm(wm).borrow_mut();
            cache.dpi_scale_add_ref(new_dpi_scale);
            cache.dpi_scale_release(state.dpi_scale.get());
            state.dpi_scale.set(new_dpi_scale);
        }
    }));
}

static CACHE: MtLock<RefCell<Cache>> = MtLock::new(RefCell::new(Cache::new()));

//
//  Cache -------+-----------------,
//               |                 |
//               v                 v
//         CacheDpiScale     CacheDpiScale
//               |                 | (bmps)
//               |        ,------, |     (bmps/img)
//      ,------, | ,------|------|-|-->,-----> CacheImg
//      |      v v |      |      v v   v
//      | ,-> CacheBmp <--|---> CacheBmp <-, (link_img)
//      | '---------------|----------------'
//      |        ^        |        ^
//      |        | ,------|--------|-->,-----> CacheImg
//      |        v |      |        v   v
//      | ,-> CacheBmp <--|---> CacheBmp <-,
//      | '------^--------|--------^-------'
//      |        |        |        |
//      '--------'        '--------' (link_dpi_scale)
//
// Assumptions:
//
//  - The number of the elements is very small - it's usually 1 or 2 and bounded
//    by the number of computer monitors connected to the user's machine.
//
#[derive(Debug)]
struct Cache {
    imgs: Pool<CacheImg>,
    bmps: Pool<CacheBmp>,
    // Mappings from `DpiScale` to `CacheDpiScale`. Hashtables would be overkill
    // for such a small number of elements.
    dpi_scales: Vec<CacheDpiScale>,
}

/// A known DPI scale.
#[derive(Debug)]
struct CacheDpiScale {
    dpi_scale: DpiScale,
    /// The number of the clients that may request rasterization for this DPI
    /// scale value. When this hits zero, all cached bitmaps in `bmps` are
    /// destroyed.
    ref_count: usize,
    /// A linked-list of `CacheBmp` having this DPI scale.
    /// Elements are linked by `CacheBmp::link_dpi_scale`.
    bmps: intrusive_list::ListHead,
}

/// An index into `Cache::dpi_scales`. Invalidated whenever the list is updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CacheDpiScalePtr(usize);

#[derive(Debug)]
struct CacheImg {
    /// A linked-list of `CacheBmp` associated with this image.
    /// Elements are linked by `CacheBmp::link_img`.
    bmps: intrusive_list::ListHead,
}

#[derive(Debug)]
struct CacheBmp {
    /// A pointer to a `CacheImg` in `Cache::imgs`.
    img: PoolPtr,
    dpi_scale: DpiScale,
    bmp: Bmp,
    /// Links `CacheBmp`s associated with the same `ImgInner` together.
    link_img: Option<intrusive_list::Link>,
    /// Links `CacheBmp`s having the same `DpiScale` together.
    link_dpi_scale: Option<intrusive_list::Link>,
}

impl Cache {
    const fn new() -> Self {
        Self {
            imgs: Pool::new(),
            bmps: Pool::new(),
            dpi_scales: Vec::new(),
        }
    }

    fn dpi_scale_add_ref(&mut self, dpi_scale: DpiScale) {
        for cache_dpi_scale in self.dpi_scales.iter_mut() {
            if cache_dpi_scale.dpi_scale == dpi_scale {
                cache_dpi_scale.ref_count += 1;
                return;
            }
        }
        self.dpi_scales.push(CacheDpiScale {
            dpi_scale,
            ref_count: 1,
            bmps: Default::default(),
        });
    }

    fn dpi_scale_release(&mut self, dpi_scale: DpiScale) {
        let i = self.dpi_scale_find(dpi_scale).map(|ptr| ptr.0);

        let i = i.expect("unknown DPI scale value");

        {
            let cache_dpi_scale = &mut self.dpi_scales[i];
            cache_dpi_scale.ref_count -= 1;

            if cache_dpi_scale.ref_count > 0 {
                return;
            }

            // `ref_count` hit zero, destroy all associated bitmmaps
            if let Some(mut bmp_ptr) = cache_dpi_scale.bmps.first {
                // Iterate through elements in a circular linked list.
                let first_bmp_ptr = bmp_ptr;
                loop {
                    let (next, img_ptr);
                    {
                        let bmp = &self.bmps[bmp_ptr];
                        next = bmp.link_dpi_scale.unwrap().next;
                        img_ptr = bmp.img;
                    }

                    // Remove the `CacheBmp` from `CacheImg::bmps`
                    let img: &mut CacheImg = &mut self.imgs[img_ptr];
                    img.bmps
                        .accessor_mut(&mut self.bmps, |bmp| &mut bmp.link_img)
                        .remove(bmp_ptr);

                    // No need to unlink `link_dpi_scale`; all bitmaps in the list
                    // are deleted in this loop anyway

                    // Delete `CacheBmp`
                    self.bmps.deallocate(bmp_ptr);

                    // Find the next bitmap
                    if next == first_bmp_ptr {
                        break;
                    } else {
                        bmp_ptr = next;
                    }
                }
            }
        }

        // Delete `CacheDpiScale`
        self.dpi_scales.swap_remove(i);
    }

    fn dpi_scale_find(&mut self, dpi_scale: DpiScale) -> Option<CacheDpiScalePtr> {
        self.dpi_scales
            .iter()
            .enumerate()
            .position(|(_, e)| e.dpi_scale == dpi_scale)
            .map(CacheDpiScalePtr)
    }

    fn img_add(&mut self) -> PoolPtr {
        self.imgs.allocate(CacheImg {
            bmps: Default::default(),
        })
    }

    fn img_remove(&mut self, img: PoolPtr) {
        // Destroy all associated bitmmaps
        if let Some(mut bmp_ptr) = self.imgs[img].bmps.first {
            // Iterate through elements in a circular linked list.
            let first_bmp_ptr = bmp_ptr;
            loop {
                let (next, dpi_scale);
                {
                    let bmp = &self.bmps[bmp_ptr];
                    next = bmp.link_img.unwrap().next;
                    dpi_scale = bmp.dpi_scale;
                }

                // Remove the `CacheBmp` from `CacheDpiScale::bmps`
                let cache_dpi_scale = self
                    .dpi_scales
                    .iter_mut()
                    .find(|e| e.dpi_scale == dpi_scale)
                    .unwrap();
                cache_dpi_scale
                    .bmps
                    .accessor_mut(&mut self.bmps, |bmp| &mut bmp.link_dpi_scale)
                    .remove(bmp_ptr);

                // No need to unlink `link_img`; all bitmaps in the list
                // are deleted in this loop anyway

                // Delete `CacheBmp`
                self.bmps.deallocate(bmp_ptr);

                // Find the next bitmap
                if next == first_bmp_ptr {
                    break;
                } else {
                    bmp_ptr = next;
                }
            }
        }

        self.imgs.deallocate(img);
    }

    fn img_find_bmp(&self, img: PoolPtr, dpi_scale: DpiScale) -> Option<&Bmp> {
        let cache_img = &self.imgs[img];

        let bmps = cache_img.bmps.accessor(&self.bmps, |bmp| &bmp.link_img);

        bmps.iter()
            .find(|(_, cache_bmp)| cache_bmp.dpi_scale == dpi_scale)
            .map(|(_, cache_bmp)| &cache_bmp.bmp)
    }

    fn img_add_bmp(&mut self, img: PoolPtr, dpi_scale: CacheDpiScalePtr, bmp: Bmp) -> &Bmp {
        let bmp_ptr = self.bmps.allocate(CacheBmp {
            img,
            dpi_scale: self.dpi_scales[dpi_scale.0].dpi_scale,
            bmp,
            link_img: None,
            link_dpi_scale: None,
        });

        // Add `bmp_ptr` to `CacheDpiScale::bmps`
        self.dpi_scales[dpi_scale.0]
            .bmps
            .accessor_mut(&mut self.bmps, |bmp| &mut bmp.link_dpi_scale)
            .push_back(bmp_ptr);

        // Add `bmp_ptr` to `CacheImg::bmps`
        self.imgs[img]
            .bmps
            .accessor_mut(&mut self.bmps, |bmp| &mut bmp.link_img)
            .push_back(bmp_ptr);

        &self.bmps[bmp_ptr].bmp
    }
}

/// A validated DPI scale value, fully supporting `Eq` and `Hash`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct DpiScale(u32);

quick_error! {
    #[derive(Debug)]
    enum DpiScaleError {
        OutOfRange {}
    }
}

impl DpiScale {
    fn new(x: f32) -> Result<Self, DpiScaleError> {
        if x.is_finite() && x > 0.0 {
            Ok(Self(x.to_bits()))
        } else {
            Err(DpiScaleError::OutOfRange)
        }
    }

    fn value(&self) -> f32 {
        <f32>::from_bits(self.0)
    }
}

impl fmt::Debug for DpiScale {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("DpiScale").field(&self.value()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::super::BitmapImg;
    use super::*;
    use crate::pal::prelude::*;

    #[test]
    fn dpi_scales() {
        let mut cache = Cache::new();

        let scale1 = DpiScale::new(1.0).unwrap();
        let scale2 = DpiScale::new(2.0).unwrap();

        assert_eq!(cache.dpi_scales.len(), 0);
        assert!(cache.dpi_scale_find(scale1).is_none());
        assert!(cache.dpi_scale_find(scale2).is_none());

        cache.dpi_scale_add_ref(scale1);
        assert_eq!(cache.dpi_scales.len(), 1);
        assert!(cache.dpi_scale_find(scale1).is_some());
        assert!(cache.dpi_scale_find(scale2).is_none());

        cache.dpi_scale_add_ref(scale2);
        assert_eq!(cache.dpi_scales.len(), 2);
        assert!(cache.dpi_scale_find(scale1).is_some());
        assert!(cache.dpi_scale_find(scale2).is_some());

        cache.dpi_scale_add_ref(scale2);
        assert_eq!(cache.dpi_scales.len(), 2);
        assert!(cache.dpi_scale_find(scale1).is_some());
        assert!(cache.dpi_scale_find(scale2).is_some());

        cache.dpi_scale_release(scale1);
        assert_eq!(cache.dpi_scales.len(), 1);
        assert!(cache.dpi_scale_find(scale1).is_none());
        assert!(cache.dpi_scale_find(scale2).is_some());

        cache.dpi_scale_release(scale2);
        assert_eq!(cache.dpi_scales.len(), 1);
        assert!(cache.dpi_scale_find(scale1).is_none());
        assert!(cache.dpi_scale_find(scale2).is_some());

        cache.dpi_scale_release(scale2);
        assert_eq!(cache.dpi_scales.len(), 0);
        assert!(cache.dpi_scale_find(scale1).is_none());
        assert!(cache.dpi_scale_find(scale2).is_none());
    }

    #[test]
    fn imgs() {
        let mut cache = Cache::new();

        let bmp = crate::pal::BitmapBuilder::new([1, 1]).into_bitmap();
        let bmp = BitmapImg::new(bmp, 1.0);

        let scale1 = DpiScale::new(1.0).unwrap();
        let scale2 = DpiScale::new(2.0).unwrap();

        let img_ptr = cache.img_add();
        assert!(cache.img_find_bmp(img_ptr, scale1).is_none());
        assert!(cache.img_find_bmp(img_ptr, scale2).is_none());

        cache.dpi_scale_add_ref(scale1);
        cache.dpi_scale_add_ref(scale2);
        let scale1ptr = cache.dpi_scale_find(scale1).unwrap();
        let scale2ptr = cache.dpi_scale_find(scale2).unwrap();

        cache.img_add_bmp(img_ptr, scale1ptr, bmp.new_bmp(1.0));
        assert!(cache.img_find_bmp(img_ptr, scale1).is_some());
        assert!(cache.img_find_bmp(img_ptr, scale2).is_none());

        cache.img_add_bmp(img_ptr, scale2ptr, bmp.new_bmp(2.0));
        assert!(cache.img_find_bmp(img_ptr, scale1).is_some());
        assert!(cache.img_find_bmp(img_ptr, scale2).is_some());

        assert_eq!(cache.bmps.iter().count(), 2);

        cache.dpi_scale_release(scale2);

        assert!(cache.img_find_bmp(img_ptr, scale1).is_some());
        assert!(cache.img_find_bmp(img_ptr, scale2).is_none());

        assert_eq!(cache.bmps.iter().count(), 1);
    }
}