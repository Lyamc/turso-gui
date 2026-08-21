//! Host integration: console attach on Windows, and on-screen window placement.

const PREFERRED_WIDTH: u32 = 1280;
const PREFERRED_HEIGHT: u32 = 800;
const MIN_WIDTH: u32 = 800;
const MIN_HEIGHT: u32 = 500;
const MARGIN: u32 = 32;

/// Pixel placement for a new app window, fully inside the working area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowPlacement {
    /// Physical screen x (Windows pixels).
    pub x: i32,
    /// Physical screen y (Windows pixels).
    pub y: i32,
    /// Physical width.
    pub width: u32,
    /// Physical height.
    pub height: u32,
    /// Logical x for DPI-aware toolkits (winit, egui, dioxus, gpui).
    pub logical_x: f32,
    /// Logical y.
    pub logical_y: f32,
    /// Logical width.
    pub logical_width: f32,
    /// Logical height.
    pub logical_height: f32,
    /// Logical minimum width (clamped to the work area).
    pub min_logical_width: f32,
    /// Logical minimum height (clamped to the work area).
    pub min_logical_height: f32,
    /// Physical minimum width (clamped to the work area).
    pub min_width: u32,
    /// Physical minimum height (clamped to the work area).
    pub min_height: u32,
}

impl WindowPlacement {
    /// Preferred size, shrunk and centered so the window stays on-screen
    /// (work area = screen minus taskbar and other reserved strips).
    pub fn suggested() -> Self {
        #[cfg(windows)]
        {
            if let Some(work) = windows::work_area() {
                return Self::from_work_area(
                    work.left,
                    work.top,
                    work.width(),
                    work.height(),
                    windows::dpi_scale(),
                );
            }
        }
        Self::from_work_area(0, 0, 1920, 1040, 1.0)
    }

    /// Fit `PREFERRED_*` into a work rectangle described in physical pixels.
    pub fn from_work_area(
        work_x: i32,
        work_y: i32,
        work_w: u32,
        work_h: u32,
        scale: f64,
    ) -> Self {
        let scale = if scale.is_finite() && scale > 0.05 {
            scale
        } else {
            1.0
        };

        let (phys_w, phys_h, phys_x, phys_y) =
            fit_rect(work_x, work_y, work_w, work_h, PREFERRED_WIDTH, PREFERRED_HEIGHT, MIN_WIDTH, MIN_HEIGHT, MARGIN);

        let logical_width = (phys_w as f64 / scale) as f32;
        let logical_height = (phys_h as f64 / scale) as f32;
        let logical_x = (phys_x as f64 / scale) as f32;
        let logical_y = (phys_y as f64 / scale) as f32;
        let min_width = MIN_WIDTH.min(phys_w).max(1);
        let min_height = MIN_HEIGHT.min(phys_h).max(1);
        let min_logical_width = ((min_width as f64 / scale) as f32).min(logical_width).max(1.0);
        let min_logical_height = ((min_height as f64 / scale) as f32)
            .min(logical_height)
            .max(1.0);

        Self {
            x: phys_x,
            y: phys_y,
            width: phys_w,
            height: phys_h,
            logical_x,
            logical_y,
            logical_width,
            logical_height,
            min_logical_width,
            min_logical_height,
            min_width,
            min_height,
        }
    }

    /// Tk `wm geometry` string using physical pixels.
    pub fn tk_geometry(&self) -> String {
        format!("{}x{}+{}+{}", self.width, self.height, self.x, self.y)
    }
}

fn fit_rect(
    work_x: i32,
    work_y: i32,
    work_w: u32,
    work_h: u32,
    preferred_w: u32,
    preferred_h: u32,
    min_w: u32,
    min_h: u32,
    margin: u32,
) -> (u32, u32, i32, i32) {
    let usable_w = work_w.saturating_sub(margin.saturating_mul(2)).max(1);
    let usable_h = work_h.saturating_sub(margin.saturating_mul(2)).max(1);
    let width = preferred_w.min(usable_w).max(min_w.min(usable_w));
    let height = preferred_h.min(usable_h).max(min_h.min(usable_h));
    let extra_x = work_w.saturating_sub(width);
    let extra_y = work_h.saturating_sub(height);
    let x = work_x.saturating_add((extra_x / 2) as i32);
    let y = work_y.saturating_add((extra_y / 2) as i32);
    (width, height, x, y)
}

/// True when the process was started with `--console`.
pub fn console_flag_present() -> bool {
    std::env::args().any(|a| a == "--console")
}

/// Attach to an existing terminal, or open one when `--console` is passed.
pub fn init_gui_host() {
    setup_console(console_flag_present());
}

/// Show a blocking error dialog when there is no console to print to.
pub fn report_error(title: &str, message: &str) {
    eprintln!("{title}: {message}");
    #[cfg(windows)]
    windows::message_box(title, message);
}

/// Attach to the parent terminal when one exists. Allocate a new console only
/// when `force` is set (for `--console`, or CLI mode with no parent).
///
/// Does nothing when stdout is already a pipe or file, so `cargo test` and
/// redirected invocations keep capturing output.
pub fn setup_console(force: bool) {
    #[cfg(windows)]
    windows::setup(force);
    #[cfg(not(windows))]
    {
        let _ = force;
    }
}

#[cfg(windows)]
mod windows {
    use std::ptr;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    impl Rect {
        pub fn width(self) -> u32 {
            self.right.saturating_sub(self.left).max(0) as u32
        }
        pub fn height(self) -> u32 {
            self.bottom.saturating_sub(self.top).max(0) as u32
        }
    }

    const SPI_GETWORKAREA: u32 = 0x0030;
    const PROCESS_PER_MONITOR_DPI_AWARE: u32 = 2;
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;
    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
    const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;
    const FILE_TYPE_DISK: u32 = 1;
    const FILE_TYPE_CHAR: u32 = 2;
    const FILE_TYPE_PIPE: u32 = 3;
    const INVALID_HANDLE_VALUE: isize = -1;

    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(
            action: u32,
            ui_param: u32,
            pv_param: *mut Rect,
            f_win_ini: u32,
        ) -> i32;
        fn GetDpiForSystem() -> u32;
        fn MessageBoxW(
            hwnd: *mut std::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            utype: u32,
        ) -> i32;
    }

    #[link(name = "shcore")]
    extern "system" {
        fn SetProcessDpiAwareness(value: u32) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn AllocConsole() -> i32;
        fn GetStdHandle(handle: u32) -> *mut std::ffi::c_void;
        fn GetFileType(handle: *mut std::ffi::c_void) -> u32;
    }

    #[repr(C)]
    struct CFile {
        _private: [u8; 0],
    }

    extern "C" {
        fn freopen_s(
            stream: *mut *mut CFile,
            filename: *const u8,
            mode: *const u8,
            old: *mut CFile,
        ) -> i32;
        fn __acrt_iob_func(idx: u32) -> *mut CFile;
    }

    fn enable_dpi() {
        unsafe {
            let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
        }
    }

    pub fn dpi_scale() -> f64 {
        enable_dpi();
        unsafe {
            let dpi = GetDpiForSystem();
            if dpi == 0 {
                1.0
            } else {
                dpi as f64 / 96.0
            }
        }
    }

    pub fn work_area() -> Option<Rect> {
        enable_dpi();
        unsafe {
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect, 0) != 0
                && rect.width() > 0
                && rect.height() > 0
            {
                Some(rect)
            } else {
                None
            }
        }
    }

    fn stdout_already_connected() -> bool {
        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle.is_null() || handle as isize == INVALID_HANDLE_VALUE {
                return false;
            }
            matches!(
                GetFileType(handle),
                FILE_TYPE_DISK | FILE_TYPE_CHAR | FILE_TYPE_PIPE
            )
        }
    }

    fn redirect_stdio() {
        unsafe {
            let mut dummy: *mut CFile = ptr::null_mut();
            let _ = freopen_s(
                &mut dummy,
                b"CONOUT$\0".as_ptr(),
                b"w\0".as_ptr(),
                __acrt_iob_func(1),
            );
            dummy = ptr::null_mut();
            let _ = freopen_s(
                &mut dummy,
                b"CONOUT$\0".as_ptr(),
                b"w\0".as_ptr(),
                __acrt_iob_func(2),
            );
            dummy = ptr::null_mut();
            let _ = freopen_s(
                &mut dummy,
                b"CONIN$\0".as_ptr(),
                b"r\0".as_ptr(),
                __acrt_iob_func(0),
            );
            let _ = GetStdHandle(STD_INPUT_HANDLE);
            let _ = GetStdHandle(STD_OUTPUT_HANDLE);
            let _ = GetStdHandle(STD_ERROR_HANDLE);
        }
    }

    pub fn message_box(title: &str, message: &str) {
        use std::os::windows::ffi::OsStrExt;

        fn wide(s: &str) -> Vec<u16> {
            std::ffi::OsStr::new(s)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        }

        let title = wide(title);
        let message = wide(message);
        unsafe {
            let _ = MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                0x10,
            );
        }
    }

    pub fn setup(force: bool) {
        if stdout_already_connected() {
            return;
        }
        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
                redirect_stdio();
                return;
            }
            if force && AllocConsole() != 0 {
                redirect_stdio();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_fits_typical_desktop() {
        let p = WindowPlacement::from_work_area(0, 0, 1920, 1040, 1.0);
        assert_eq!(p.width, 1280);
        assert_eq!(p.height, 800);
        assert_eq!(p.x, (1920 - 1280) / 2);
        assert_eq!(p.y, (1040 - 800) / 2);
        assert_eq!(p.logical_width, 1280.0);
        assert!(p.x >= 0 && p.y >= 0);
        assert!(p.x as u32 + p.width <= 1920);
        assert!(p.y as u32 + p.height <= 1040);
    }

    #[test]
    fn shrinks_to_small_work_area() {
        let p = WindowPlacement::from_work_area(0, 40, 1366, 728, 1.0);
        assert!(p.width <= 1366 - MARGIN * 2);
        assert!(p.height <= 728 - MARGIN * 2);
        assert!(p.y >= 40);
        assert!((p.y as u32) + p.height <= 40 + 728);
        assert!(p.x >= 0);
        assert!(p.x as u32 + p.width <= 1366);
    }

    #[test]
    fn stays_inside_tiny_display() {
        let p = WindowPlacement::from_work_area(0, 0, 800, 500, 1.0);
        assert!(p.width <= 800);
        assert!(p.height <= 500);
        assert!(p.x >= 0 && p.y >= 0);
        assert!(p.x as u32 + p.width <= 800);
        assert!(p.y as u32 + p.height <= 500);
    }

    #[test]
    fn logical_size_accounts_for_dpi() {
        let p = WindowPlacement::from_work_area(0, 0, 1920, 1080, 1.5);
        assert!((p.logical_width - p.width as f32 / 1.5).abs() < 1.0);
        assert!(p.logical_height <= 1080.0 / 1.5);
        assert!(p.min_logical_width <= p.logical_width);
        assert!(p.min_logical_height <= p.logical_height);
    }

    #[test]
    fn tk_geometry_is_wxh_plus_origin() {
        let p = WindowPlacement::from_work_area(10, 20, 1920, 1040, 1.0);
        assert!(p.tk_geometry().contains('x'));
        assert!(p.tk_geometry().contains('+'));
    }
}
