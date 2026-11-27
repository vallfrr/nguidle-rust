//! The window only takes a few fixed sizes, all with the proportions of the 960 x 600 view.

/// Client sizes the window may have, smallest first.
pub const SIZES: [(u32, u32); 8] = [
    (640, 400),
    (768, 480),
    (960, 600),
    (1152, 720),
    (1280, 800),
    (1440, 900),
    (1680, 1050),
    (1920, 1200),
];

/// The allowed size whose width is closest to `width`.
pub fn nearest(width: f32) -> (u32, u32) {
    *SIZES
        .iter()
        .min_by(|a, b| {
            (a.0 as f32 - width)
                .abs()
                .total_cmp(&(b.0 as f32 - width).abs())
        })
        .expect("sizes")
}

/// Makes the window follow `SIZES` while the player drags its edge (Windows only: the
/// system lets a program adjust the rectangle during the drag; other systems get the
/// correction once the drag ends, from `resize_event`).
#[cfg(windows)]
pub fn lock_while_dragging(title: &str) {
    win::install(title);
}

#[cfg(not(windows))]
pub fn lock_while_dragging(_title: &str) {}

#[cfg(windows)]
#[allow(unsafe_code)]
mod win {
    use super::nearest;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicIsize, Ordering};

    type Hwnd = *mut c_void;
    const GWLP_WNDPROC: i32 = -4;
    const WM_SIZING: u32 = 0x0214;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetActiveWindow() -> Hwnd;
        fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
        fn SetWindowLongPtrW(window: Hwnd, index: i32, value: isize) -> isize;
        fn CallWindowProcW(prev: isize, window: Hwnd, msg: u32, w: usize, l: isize) -> isize;
        fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
        fn GetClientRect(window: Hwnd, rect: *mut Rect) -> i32;
    }

    static PREVIOUS: AtomicIsize = AtomicIsize::new(0);

    pub fn install(title: &str) {
        // SAFETY: plain Win32 calls on this thread's own window; the replacement procedure
        // forwards every other message to the previous one.
        unsafe {
            let mut window = GetActiveWindow();
            if window.is_null() {
                let wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
                window = FindWindowW(std::ptr::null(), wide.as_ptr());
            }
            if window.is_null() {
                return;
            }
            let previous = SetWindowLongPtrW(window, GWLP_WNDPROC, procedure as *const () as usize as isize);
            PREVIOUS.store(previous, Ordering::SeqCst);
        }
    }

    unsafe extern "system" fn procedure(window: Hwnd, msg: u32, w: usize, l: isize) -> isize {
        if msg == WM_SIZING && l != 0 {
            // SAFETY: for WM_SIZING, `l` points to the rectangle the system is about to apply.
            let rect = &mut *(l as *mut Rect);
            let (mut outer, mut inner) = (Rect::default(), Rect::default());
            GetWindowRect(window, &mut outer);
            GetClientRect(window, &mut inner);
            let frame_w = (outer.right - outer.left) - inner.right;
            let frame_h = (outer.bottom - outer.top) - inner.bottom;
            let edge = w as u32;
            // 1 left, 2 right, 3 top, 4 top left, 5 top right, 6 bottom, 7 bottom left, 8 bottom right.
            let by_height = matches!(edge, 3 | 6);
            let asked_w = (rect.right - rect.left - frame_w) as f32;
            let asked_h = (rect.bottom - rect.top - frame_h) as f32;
            let (cw, ch) = nearest(if by_height {
                asked_h * 960.0 / 600.0
            } else {
                asked_w
            });
            let (ww, hh) = (cw as i32 + frame_w, ch as i32 + frame_h);
            if matches!(edge, 1 | 4 | 7) {
                rect.left = rect.right - ww;
            } else {
                rect.right = rect.left + ww;
            }
            if matches!(edge, 3 | 4 | 5) {
                rect.top = rect.bottom - hh;
            } else {
                rect.bottom = rect.top + hh;
            }
            return 1;
        }
        CallWindowProcW(PREVIOUS.load(Ordering::SeqCst), window, msg, w, l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_size_keeps_the_proportions_of_the_view() {
        for (w, h) in SIZES {
            assert_eq!(w * 600, h * 960, "{w}x{h}");
        }
    }

    #[test]
    fn a_dragged_width_snaps_to_the_closest_size() {
        assert_eq!(nearest(10.0), (640, 400));
        assert_eq!(nearest(1000.0), (960, 600));
        assert_eq!(nearest(1100.0), (1152, 720));
        assert_eq!(nearest(5000.0), (1920, 1200));
    }
}
