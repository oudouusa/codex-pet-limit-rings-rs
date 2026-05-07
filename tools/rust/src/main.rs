#![windows_subsystem = "windows"]
#![allow(unsafe_op_in_unsafe_fn)]

use std::{
    cell::Cell,
    env,
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime},
};

use rusqlite::Connection;
use serde_json::Value;
use tiny_skia::{
    Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, PixmapMut, Stroke, Transform,
};
use windows::{
    Win32::{
        Foundation::{
            BOOL, COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
        },
        Graphics::Gdi::{
            ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
            CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW,
            DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
            DeleteDC, DeleteObject, DrawTextW, FF_SWISS, FW_SEMIBOLD, HGDIOBJ,
            MONITOR_DEFAULTTONEAREST, MonitorFromPoint, OUT_DEFAULT_PRECIS, SelectObject,
            SetBkMode, SetTextColor, TRANSPARENT,
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            ProcessStatus::EmptyWorkingSet,
            Threading::{
                GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
                SetProcessDpiAwarenessContext,
            },
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreatePopupMenu,
                CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW, EnumWindows,
                GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW,
                GetWindowRect, GetWindowThreadProcessId, HMENU, IDC_ARROW, IDI_APPLICATION,
                IsWindow, IsWindowVisible, KillTimer, LoadIconW, MF_CHECKED, MF_GRAYED,
                MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSG, PostQuitMessage, RegisterClassW,
                SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
                SWP_NOSENDCHANGING, SWP_NOZORDER, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
                SetWindowPos, ShowWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TrackPopupMenu,
                TranslateMessage, WM_APP, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_HOTKEY,
                WM_RBUTTONUP, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
                WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
            },
        },
    },
    core::{PCWSTR, PWSTR, w},
};

const WM_TRAY: u32 = WM_APP + 1;
const TIMER_FRAME: usize = 1;
const TIMER_STATE: usize = 2;
const TIMER_ANIMATION: usize = 3;
const TIMER_TRIM: usize = 4;
const HOTKEY_ALIGN: i32 = 0x5247;
const FRAME_FAST_MS: u32 = 16;
const FRAME_IDLE_MS: u32 = 80;
const DRAG_HIT_PADDING: i32 = 42;
const LIVE_FOLLOW_MS: u64 = 1600;
const RELEASE_FOLLOW_MS: u64 = 2600;
const RELEASE_LIVE_FOLLOW_MS: u64 = 900;
const STABILIZE_ANCHOR_TOLERANCE: i32 = 4;
const STABILIZE_JUMP_PX: i32 = 18;

const CMD_SHOW: usize = 101;
const CMD_FALLBACK: usize = 102;
const CMD_REFRESH: usize = 103;
const CMD_LEFT: usize = 104;
const CMD_RIGHT: usize = 105;
const CMD_UP: usize = 106;
const CMD_DOWN: usize = 107;
const CMD_RESET: usize = 108;
const CMD_QUIT: usize = 109;
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;

unsafe extern "system" {
    fn RegisterHotKey(hwnd: HWND, id: i32, fs_modifiers: u32, vk: u32) -> BOOL;
    fn UnregisterHotKey(hwnd: HWND, id: i32) -> BOOL;
    fn GetAsyncKeyState(vkey: i32) -> i16;
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct Rect {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

impl Rect {
    fn right(self) -> i32 {
        self.left + self.width
    }

    fn bottom(self) -> i32 {
        self.top + self.height
    }

    fn center(self) -> (i32, i32) {
        (self.left + self.width / 2, self.top + self.height / 2)
    }

    fn contains(self, point: POINT, padding: i32) -> bool {
        point.x >= self.left - padding
            && point.x <= self.right() + padding
            && point.y >= self.top - padding
            && point.y <= self.bottom() + padding
    }
}

fn null_hwnd() -> HWND {
    HWND(null_mut())
}

fn is_null_hwnd(hwnd: HWND) -> bool {
    hwnd.0.is_null()
}

#[derive(Clone, Copy, Debug)]
struct LimitBucket {
    used_percent: f64,
}

impl LimitBucket {
    fn remaining_percent(self) -> f64 {
        (100.0 - self.used_percent).clamp(0.0, 100.0)
    }
}

#[derive(Clone, Debug)]
struct LimitState {
    primary: Option<LimitBucket>,
    secondary: Option<LimitBucket>,
    source: &'static str,
}

impl Default for LimitState {
    fn default() -> Self {
        Self {
            primary: None,
            secondary: None,
            source: "none",
        }
    }
}

#[derive(Clone, Copy)]
struct ColorStop {
    remaining_at_or_below: f64,
    color: Rgba,
}

#[derive(Clone, Copy)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgba {
    const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    fn to_skia(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

#[derive(Clone)]
struct VisualOptions {
    primary: Rgba,
    secondary: Rgba,
    alerts: [ColorStop; 2],
}

impl Default for VisualOptions {
    fn default() -> Self {
        Self {
            primary: Rgba::new(61, 235, 189, 245),
            secondary: Rgba::new(92, 179, 255, 230),
            alerts: [
                ColorStop {
                    remaining_at_or_below: 12.0,
                    color: Rgba::new(255, 66, 56, 245),
                },
                ColorStop {
                    remaining_at_or_below: 30.0,
                    color: Rgba::new(255, 173, 51, 245),
                },
            ],
        }
    }
}

impl VisualOptions {
    fn color_for(&self, remaining: f64, primary: bool) -> Rgba {
        for stop in self.alerts {
            if remaining <= stop.remaining_at_or_below {
                return stop.color;
            }
        }
        if primary {
            self.primary
        } else {
            self.secondary
        }
    }
}

#[derive(Clone)]
struct Config {
    state_path: PathBuf,
    logs_path: PathBuf,
    auth_path: PathBuf,
    preview_path: Option<PathBuf>,
    size: i32,
    show_without_pet: bool,
    offset_x: i32,
    offset_y: i32,
    scale_x: f64,
    scale_y: f64,
    visual: VisualOptions,
}

#[derive(Clone, Copy)]
struct PetFrameSnapshot {
    overlay: Rect,
    mascot: Rect,
    anchor: Rect,
}

#[derive(Clone, Copy)]
struct DragFollow {
    grab_x: i32,
    grab_y: i32,
    width: i32,
    height: i32,
}

struct PetFrameReader {
    path: PathBuf,
    cached_modified: Option<SystemTime>,
    cached: Option<PetFrameSnapshot>,
}

impl PetFrameReader {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            cached_modified: None,
            cached: None,
        }
    }

    fn read_pet_frame(&mut self) -> Option<Rect> {
        self.read_snapshot().map(|snapshot| snapshot.anchor)
    }

    fn read_snapshot(&mut self) -> Option<PetFrameSnapshot> {
        let modified = fs::metadata(&self.path).and_then(|m| m.modified()).ok();
        if modified.is_some() && modified == self.cached_modified {
            return self.cached;
        }

        let text = fs::read_to_string(&self.path).ok()?;
        let root: Value = serde_json::from_str(&text).ok()?;
        let roots = [Some(&root), root.get("electron-persisted-atom-state")];

        for search_root in roots.into_iter().flatten() {
            if !overlay_open(search_root) {
                continue;
            }
            let bounds = match search_root.get("electron-avatar-overlay-bounds") {
                Some(value) if value.is_object() => value,
                _ => continue,
            };
            let mascot = match bounds.get("mascot") {
                Some(value) if value.is_object() => value,
                _ => continue,
            };

            let x = number(bounds, "x")?;
            let y = number(bounds, "y")?;
            let overlay_width = number(bounds, "width")?;
            let overlay_height = number(bounds, "height")?;
            let left = number(mascot, "left")?;
            let top = number(mascot, "top")?;
            let width = number(mascot, "width")?;
            let height = number(mascot, "height")?;

            let mascot = Rect {
                left: left.round() as i32,
                top: top.round() as i32,
                width: (width.round() as i32).max(1),
                height: (height.round() as i32).max(1),
            };
            let fallback = Rect {
                left: (x + left).round() as i32,
                top: (y + top).round() as i32,
                width: mascot.width,
                height: mascot.height,
            };
            let anchor = bounds
                .get("anchor")
                .and_then(rect_from_value)
                .unwrap_or(fallback);
            let snapshot = PetFrameSnapshot {
                overlay: Rect {
                    left: x.round() as i32,
                    top: y.round() as i32,
                    width: (overlay_width.round() as i32).max(1),
                    height: (overlay_height.round() as i32).max(1),
                },
                mascot,
                anchor,
            };
            self.cached_modified = modified;
            self.cached = Some(snapshot);
            return Some(snapshot);
        }

        self.cached_modified = modified;
        self.cached = None;
        None
    }
}

fn overlay_open(root: &Value) -> bool {
    match root.get("electron-avatar-overlay-open") {
        Some(Value::Bool(false)) => false,
        Some(Value::Number(n)) => n.as_i64() != Some(0),
        _ => true,
    }
}

fn rect_from_value(value: &Value) -> Option<Rect> {
    Some(Rect {
        left: number(value, "x")?.round() as i32,
        top: number(value, "y")?.round() as i32,
        width: (number(value, "width")?.round() as i32).max(1),
        height: (number(value, "height")?.round() as i32).max(1),
    })
}

fn number(value: &Value, name: &str) -> Option<f64> {
    value.get(name)?.as_f64()
}

#[derive(Default)]
struct PetOverlayWindowTracker {
    handle: HWND,
    next_search: InstantCell,
    last_bounds: Option<Rect>,
}

impl PetOverlayWindowTracker {
    fn read_live_overlay_frame(&mut self, expected: Rect) -> Option<Rect> {
        if let Some(bounds) = self.try_read_tracked(expected) {
            return Some(bounds);
        }

        if !self.next_search.elapsed() {
            return None;
        }

        let candidate = find_best_codex_window(expected);
        if let Some(candidate) = candidate {
            self.handle = candidate.handle;
            self.last_bounds = Some(candidate.bounds);
            self.next_search.reset_now();
            Some(candidate.bounds)
        } else {
            self.handle = null_hwnd();
            self.last_bounds = None;
            self.next_search.reset_after(Duration::from_millis(450));
            None
        }
    }

    fn current_handle(&self) -> HWND {
        self.handle
    }

    fn try_read_tracked(&mut self, expected: Rect) -> Option<Rect> {
        if is_null_hwnd(self.handle) {
            return None;
        }
        unsafe {
            if !IsWindow(self.handle).as_bool() || !IsWindowVisible(self.handle).as_bool() {
                self.handle = null_hwnd();
                self.last_bounds = None;
                return None;
            }
        }
        let bounds = window_bounds(self.handle)?;
        let still_same_window = self
            .last_bounds
            .map(|last| looks_similar_overlay_size(bounds, last))
            .unwrap_or(false);
        if looks_live_overlay_bounds(bounds)
            && (looks_same_size(bounds, expected) || still_same_window)
        {
            self.last_bounds = Some(bounds);
            Some(bounds)
        } else {
            self.handle = null_hwnd();
            self.last_bounds = None;
            None
        }
    }
}

#[derive(Clone, Copy)]
struct WindowCandidate {
    handle: HWND,
    bounds: Rect,
    score: f64,
}

fn find_best_codex_window(expected: Rect) -> Option<WindowCandidate> {
    struct Search {
        expected: Rect,
        best: Option<WindowCandidate>,
    }
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut Search);
        if !IsWindowVisible(hwnd).as_bool() {
            return true.into();
        }
        let Some(bounds) = window_bounds(hwnd) else {
            return true.into();
        };
        if !is_codex_window(hwnd) || !looks_candidate(bounds, search.expected) {
            return true.into();
        }
        let score = score_window(bounds, search.expected);
        if search.best.map(|best| score < best.score).unwrap_or(true) {
            search.best = Some(WindowCandidate {
                handle: hwnd,
                bounds,
                score,
            });
        }
        true.into()
    }

    let mut search = Search {
        expected,
        best: None,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM((&mut search as *mut Search) as isize),
        );
    }
    search.best
}

fn looks_candidate(bounds: Rect, expected: Rect) -> bool {
    if !looks_live_overlay_bounds(bounds) {
        return false;
    }
    if looks_same_size(bounds, expected) {
        return true;
    }
    let (ex, ey) = expected.center();
    let (bx, by) = bounds.center();
    let distance = (bx - ex).abs() + (by - ey).abs();
    distance < 900.max(expected.width.max(expected.height) * 4)
}

fn looks_live_overlay_bounds(bounds: Rect) -> bool {
    bounds.width >= 80 && bounds.height >= 80 && bounds.width <= 1200 && bounds.height <= 1200
}

fn looks_same_size(bounds: Rect, expected: Rect) -> bool {
    if expected.width <= 0 || expected.height <= 0 {
        return true;
    }
    let width_ratio = bounds.width as f64 / expected.width as f64;
    let height_ratio = bounds.height as f64 / expected.height as f64;
    width_ratio > 0.45 && width_ratio < 2.2 && height_ratio > 0.45 && height_ratio < 2.2
}

fn looks_similar_overlay_size(bounds: Rect, previous: Rect) -> bool {
    if previous.width <= 0 || previous.height <= 0 {
        return false;
    }
    let width_ratio = bounds.width as f64 / previous.width as f64;
    let height_ratio = bounds.height as f64 / previous.height as f64;
    width_ratio > 0.70 && width_ratio < 1.45 && height_ratio > 0.70 && height_ratio < 1.45
}

fn score_window(bounds: Rect, expected: Rect) -> f64 {
    let (bx, by) = bounds.center();
    let (ex, ey) = expected.center();
    let center_delta = (bx - ex).abs() + (by - ey).abs();
    let size_delta =
        (bounds.width - expected.width).abs() + (bounds.height - expected.height).abs();
    let area_penalty =
        (bounds.width * bounds.height - expected.width * expected.height).max(0) as f64 / 1000.0;
    center_delta as f64 * 2.0 + size_delta as f64 * 3.0 + area_penalty
}

fn is_codex_window(hwnd: HWND) -> bool {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return false;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        let Ok(handle) = handle else {
            return false;
        };
        let mut buffer = [0u16; 512];
        let mut size = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            Default::default(),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .is_ok();
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if !ok || size == 0 {
            return false;
        }
        let path = String::from_utf16_lossy(&buffer[..size as usize]).to_ascii_lowercase();
        path.ends_with("\\codex.exe") || path.ends_with("/codex.exe")
    }
}

fn window_bounds(hwnd: HWND) -> Option<Rect> {
    let mut rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        None
    } else {
        Some(Rect {
            left: rect.left,
            top: rect.top,
            width,
            height,
        })
    }
}

#[derive(Default)]
struct InstantCell(Cell<Option<Instant>>);

impl InstantCell {
    fn elapsed(&self) -> bool {
        self.0.get().map(|t| Instant::now() >= t).unwrap_or(true)
    }

    fn reset_after(&self, duration: Duration) {
        self.0.set(Some(Instant::now() + duration));
    }

    fn reset_now(&self) {
        self.0.set(None);
    }
}

struct StateReader {
    logs: PathBuf,
    auth: PathBuf,
}

impl StateReader {
    fn read_latest(self) -> LimitState {
        self.read_live().unwrap_or_else(|| self.read_log())
    }

    fn read_live(&self) -> Option<LimitState> {
        let token = read_access_token(&self.auth)?;
        let response = ureq::get("https://chatgpt.com/backend-api/wham/usage")
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/json")
            .timeout(Duration::from_secs(7))
            .call()
            .ok()?;
        if response.status() >= 300 {
            return None;
        }
        let text = response.into_string().ok()?;
        let root: Value = serde_json::from_str(&text).ok()?;
        let rate = root.get("rate_limit")?;
        Some(LimitState {
            primary: to_bucket(rate.get("primary"), rate.get("primary_window")),
            secondary: to_bucket(rate.get("secondary"), rate.get("secondary_window")),
            source: "live",
        })
    }

    fn read_log(&self) -> LimitState {
        if !self.logs.exists() {
            return LimitState::default();
        }
        let connection = match Connection::open_with_flags(
            &self.logs,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(connection) => connection,
            Err(_) => return LimitState::default(),
        };
        let body: Option<String> = connection
            .query_row(
                r#"
                SELECT feedback_log_body
                FROM logs
                WHERE feedback_log_body LIKE '%"type":"codex.rate_limits"%'
                ORDER BY ts DESC, ts_nanos DESC, id DESC
                LIMIT 1
                "#,
                [],
                |row| row.get(0),
            )
            .ok();
        let Some(body) = body else {
            return LimitState::default();
        };
        let Some(json) = extract_rate_limit_json(&body) else {
            return LimitState::default();
        };
        let Ok(root) = serde_json::from_str::<Value>(&json) else {
            return LimitState::default();
        };
        let Some(rate) = root.get("rate_limits") else {
            return LimitState::default();
        };
        LimitState {
            primary: to_bucket(rate.get("primary"), rate.get("primary_window")),
            secondary: to_bucket(rate.get("secondary"), rate.get("secondary_window")),
            source: "cached",
        }
    }
}

fn read_access_token(path: &Path) -> Option<String> {
    let root: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    find_string_property(&root, "access_token")
}

fn find_string_property(value: &Value, name: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == name {
                    if let Some(text) = value.as_str() {
                        return Some(text.to_owned());
                    }
                }
                if let Some(found) = find_string_property(value, name) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items
            .iter()
            .find_map(|item| find_string_property(item, name)),
        _ => None,
    }
}

fn to_bucket(primary: Option<&Value>, secondary: Option<&Value>) -> Option<LimitBucket> {
    [primary, secondary]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            candidate
                .get("used_percent")?
                .as_f64()
                .map(|used_percent| LimitBucket { used_percent })
        })
}

fn extract_rate_limit_json(body: &str) -> Option<String> {
    let start = body.find(r#"{"type":"codex.rate_limits""#)?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaping = false;
    for (offset, ch) in body[start..].char_indices() {
        if in_string {
            if escaping {
                escaping = false;
            } else if ch == '\\' {
                escaping = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(body[start..start + offset + ch.len_utf8()].to_owned());
                }
            }
            _ => {}
        }
    }
    None
}

struct App {
    hwnd: HWND,
    config: Config,
    pet_reader: PetFrameReader,
    tracker: PetOverlayWindowTracker,
    state: LimitState,
    state_rx: Receiver<LimitState>,
    state_tx: Sender<LimitState>,
    state_in_flight: bool,
    rings_visible: bool,
    current_pet: Option<Rect>,
    drag_follow: Option<DragFollow>,
    last_ring_bounds: Option<Rect>,
    last_show_readout: bool,
    last_state_anchor: Option<Rect>,
    last_live_overlay: Option<Rect>,
    last_pet_window: HWND,
    last_z_refresh: Instant,
    live_follow_until: Instant,
    release_follow_until: Instant,
    mouse_was_down: bool,
    start: Instant,
    phase: f64,
    summary: String,
    disposed: bool,
}

impl App {
    fn new(config: Config) -> Self {
        let (state_tx, state_rx) = mpsc::channel();
        let state_path = config.state_path.clone();
        let now = Instant::now();
        Self {
            hwnd: null_hwnd(),
            pet_reader: PetFrameReader::new(state_path),
            tracker: PetOverlayWindowTracker::default(),
            state: LimitState::default(),
            state_rx,
            state_tx,
            state_in_flight: false,
            rings_visible: true,
            current_pet: None,
            drag_follow: None,
            last_ring_bounds: None,
            last_show_readout: false,
            last_state_anchor: None,
            last_live_overlay: None,
            last_pet_window: null_hwnd(),
            last_z_refresh: now,
            live_follow_until: now - Duration::from_secs(1),
            release_follow_until: now - Duration::from_secs(1),
            mouse_was_down: false,
            start: now,
            phase: 0.0,
            summary: "Waiting for Codex limit data".to_owned(),
            disposed: false,
            config,
        }
    }

    fn run(mut self) -> windows::core::Result<()> {
        enable_dpi_awareness();
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class_name = w!("CodexPetLimitRingsRustWindow");
            let wc = WNDCLASSW {
                hCursor: windows::Win32::UI::WindowsAndMessaging::LoadCursorW(None, IDC_ARROW)?,
                hInstance: HINSTANCE(instance.0),
                lpszClassName: class_name,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                ..Default::default()
            };
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOPMOST,
                class_name,
                w!("Codex Pet Limit Rings"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1,
                1,
                None,
                None,
                HINSTANCE(instance.0),
                None,
            )?;
            self.hwnd = hwnd;
            let raw = Box::into_raw(Box::new(self));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, raw as isize);
            let app = &mut *raw;
            app.add_tray_icon();
            let _ = RegisterHotKey(hwnd, HOTKEY_ALIGN, MOD_CONTROL | MOD_ALT, 'R' as u32);
            SetTimer(hwnd, TIMER_FRAME, 80, None);
            SetTimer(hwnd, TIMER_STATE, 20_000, None);
            SetTimer(hwnd, TIMER_ANIMATION, 33, None);
            app.refresh_state_async();
            app.update_frame(false);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }

    fn add_tray_icon(&self) {
        unsafe {
            let mut data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
                ..Default::default()
            };
            write_wide_fixed(&mut data.szTip, "Codex Pet Limit Rings");
            let _ = Shell_NotifyIconW(NIM_ADD, &data);
        }
    }

    fn remove_tray_icon(&self) {
        unsafe {
            let data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
    }

    fn refresh_state_async(&mut self) {
        if self.state_in_flight {
            return;
        }
        self.state_in_flight = true;
        let reader = StateReader {
            logs: self.config.logs_path.clone(),
            auth: self.config.auth_path.clone(),
        };
        let tx = self.state_tx.clone();
        thread::spawn(move || {
            let _ = tx.send(reader.read_latest());
        });
    }

    fn poll_state(&mut self) {
        while let Ok(state) = self.state_rx.try_recv() {
            self.state = state;
            self.state_in_flight = false;
            self.update_summary();
            self.render();
            unsafe {
                SetTimer(self.hwnd, TIMER_TRIM, 2500, None);
            }
        }
    }

    fn update_summary(&mut self) {
        let mut pieces = Vec::new();
        if let Some(primary) = self.state.primary {
            pieces.push(format!(
                "Short {}",
                format_percent(primary.remaining_percent())
            ));
        }
        if let Some(secondary) = self.state.secondary {
            pieces.push(format!(
                "Weekly {}",
                format_percent(secondary.remaining_percent())
            ));
        }
        self.summary = if pieces.is_empty() {
            "Waiting for Codex limit data".to_owned()
        } else {
            format!(
                "{} {}",
                if self.state.source == "live" {
                    "Live"
                } else {
                    "Cached"
                },
                pieces.join(" | ")
            )
        };
    }

    fn update_frame(&mut self, force_fallback: bool) {
        self.poll_state();
        if !self.rings_visible {
            self.hide();
            return;
        }

        let snapshot = self.pet_reader.read_snapshot();
        let mut pet_window = null_hwnd();
        let mut state_anchor = None;
        let mut skip_stabilize = false;
        let mut physical = if let Some(snapshot) = snapshot {
            state_anchor = Some(snapshot.anchor);
            let now = Instant::now();
            let state_overlay_guess = self.transform_without_offset(snapshot.overlay);
            let state_pet_guess = self.transform(snapshot.anchor);
            let live_overlay = self.tracker.read_live_overlay_frame(state_overlay_guess);
            let cursor = cursor_pos();
            let mouse_down = left_button_down();
            let released_after_drag =
                self.mouse_was_down && !mouse_down && self.drag_follow.is_some();
            if released_after_drag {
                self.release_follow_until = now + Duration::from_millis(RELEASE_FOLLOW_MS);
                self.live_follow_until = self.release_follow_until;
            }
            skip_stabilize = mouse_down || now < self.release_follow_until;
            if mouse_down && self.drag_follow.is_none() {
                let pet = self.current_pet.unwrap_or(state_pet_guess);
                if pet.contains(cursor, DRAG_HIT_PADDING) {
                    self.drag_follow = Some(DragFollow {
                        grab_x: cursor.x - pet.left,
                        grab_y: cursor.y - pet.top,
                        width: pet.width,
                        height: pet.height,
                    });
                }
            } else if !mouse_down {
                self.drag_follow = None;
            }
            if let Some(live) = live_overlay {
                pet_window = self.tracker.current_handle();
                let live_moved = self
                    .last_live_overlay
                    .map(|previous| previous.left != live.left || previous.top != live.top)
                    .unwrap_or(false);
                if live_moved || (mouse_down && self.drag_follow.is_some()) {
                    let follow_until = if now < self.release_follow_until {
                        now + Duration::from_millis(RELEASE_LIVE_FOLLOW_MS)
                    } else {
                        now + Duration::from_millis(LIVE_FOLLOW_MS)
                    };
                    if follow_until > self.live_follow_until {
                        self.live_follow_until = follow_until;
                    }
                }
                self.last_live_overlay = Some(live);
                if let Some(drag) = self.drag_follow {
                    Rect {
                        left: cursor.x - drag.grab_x,
                        top: cursor.y - drag.grab_y,
                        width: drag.width,
                        height: drag.height,
                    }
                } else {
                    pet_frame_from_live_overlay(
                        snapshot.mascot,
                        snapshot.overlay,
                        live,
                        self.config.offset_x,
                        self.config.offset_y,
                        self.config.scale_x,
                        self.config.scale_y,
                    )
                }
            } else if let Some(drag) = self.drag_follow {
                Rect {
                    left: cursor.x - drag.grab_x,
                    top: cursor.y - drag.grab_y,
                    width: drag.width,
                    height: drag.height,
                }
            } else {
                state_pet_guess
            }
        } else if force_fallback || self.config.show_without_pet {
            fallback_pet_frame(self.config.size)
        } else {
            self.hide();
            return;
        };
        if let Some(anchor) = state_anchor {
            if !skip_stabilize {
                physical = self.stabilize_pet_frame(anchor, physical);
            }
            self.last_state_anchor = Some(anchor);
        } else {
            self.last_state_anchor = None;
        }

        self.current_pet = Some(physical);
        let visual_scale = visual_scale_for_rect(physical);
        let padding = scaled_i32(38, visual_scale);
        let size =
            (physical.width.max(physical.height) + padding * 2).max(scaled_i32(120, visual_scale));
        let ring_bounds = Rect {
            left: physical.left + physical.width / 2 - size / 2,
            top: physical.top + physical.height / 2 - size / 2,
            width: size,
            height: size,
        };
        let previous_ring_bounds = self.last_ring_bounds;
        let changed = previous_ring_bounds != Some(ring_bounds);
        self.last_ring_bounds = Some(ring_bounds);
        let show_readout = is_mouse_over(self.current_pet, ring_bounds);
        let visual_changed = self.last_show_readout != show_readout;
        let z_changed = pet_window != self.last_pet_window;
        unsafe {
            if changed || z_changed {
                let insert_after = if is_null_hwnd(pet_window) {
                    windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST
                } else {
                    pet_window
                };
                let mut flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOSENDCHANGING;
                if !z_changed {
                    flags |= SWP_NOZORDER;
                }
                let _ = SetWindowPos(
                    self.hwnd,
                    insert_after,
                    ring_bounds.left,
                    ring_bounds.top,
                    ring_bounds.width,
                    ring_bounds.height,
                    flags,
                );
                if z_changed {
                    self.last_z_refresh = Instant::now();
                    self.last_pet_window = pet_window;
                }
            }
            let _ = ShowWindow(self.hwnd, SW_SHOWNA);
        }
        if changed || visual_changed {
            self.render_with_readout(show_readout);
        }
        let frame_now = Instant::now();
        unsafe {
            SetTimer(
                self.hwnd,
                TIMER_FRAME,
                if frame_now < self.live_follow_until
                    || frame_now < self.release_follow_until
                    || left_button_down()
                {
                    FRAME_FAST_MS
                } else {
                    FRAME_IDLE_MS
                },
                None,
            );
        }
        self.mouse_was_down = left_button_down();
    }

    fn stabilize_pet_frame(&self, state_anchor: Rect, physical: Rect) -> Rect {
        let Some(previous_anchor) = self.last_state_anchor else {
            return physical;
        };
        let Some(previous_pet) = self.current_pet else {
            return physical;
        };
        let anchor_stable = (state_anchor.left - previous_anchor.left).abs()
            + (state_anchor.top - previous_anchor.top).abs()
            <= STABILIZE_ANCHOR_TOLERANCE;
        let visual_jump =
            (physical.left - previous_pet.left).abs() + (physical.top - previous_pet.top).abs();
        let jump_threshold = scaled_i32(STABILIZE_JUMP_PX, visual_scale_for_rect(physical));
        if anchor_stable && visual_jump >= jump_threshold {
            Rect {
                left: previous_pet.left,
                top: previous_pet.top,
                width: physical.width,
                height: physical.height,
            }
        } else {
            physical
        }
    }

    fn hide(&mut self) {
        self.last_ring_bounds = None;
        self.last_show_readout = false;
        self.last_state_anchor = None;
        self.drag_follow = None;
        self.mouse_was_down = false;
        self.release_follow_until = Instant::now() - Duration::from_secs(1);
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            SetTimer(self.hwnd, TIMER_TRIM, 1000, None);
        }
    }

    fn render(&mut self) {
        let Some(bounds) = self.last_ring_bounds else {
            return;
        };
        let show_readout = is_mouse_over(self.current_pet, bounds);
        self.render_with_readout(show_readout);
    }

    fn render_with_readout(&mut self, show_readout: bool) {
        let Some(bounds) = self.last_ring_bounds else {
            return;
        };
        self.last_show_readout = show_readout;
        let mut pixmap = render_rings(
            bounds.width.max(1) as u32,
            bounds.height.max(1) as u32,
            &self.state,
            self.phase,
            show_readout,
            visual_scale_for_rect(bounds),
            &self.config.visual,
        );
        let bgra = rgba_to_bgra_premul(pixmap.data_mut());
        unsafe {
            update_layered(self.hwnd, bounds, &bgra);
        }
    }

    fn should_animate(&self) -> bool {
        self.rings_visible && self.last_ring_bounds.is_some()
    }

    fn transform(&self, rect: Rect) -> Rect {
        let mut transformed = self.transform_without_offset(rect);
        transformed.left += self.config.offset_x;
        transformed.top += self.config.offset_y;
        transformed
    }

    fn transform_without_offset(&self, rect: Rect) -> Rect {
        let (dpi_scale_x, dpi_scale_y) = monitor_scale_for_rect(rect);
        let scale_x = dpi_scale_x * self.config.scale_x;
        let scale_y = dpi_scale_y * self.config.scale_y;
        let left = (rect.left as f64 * scale_x).round() as i32;
        let top = (rect.top as f64 * scale_y).round() as i32;
        let right = (rect.right() as f64 * scale_x).round() as i32;
        let bottom = (rect.bottom() as f64 * scale_y).round() as i32;
        Rect {
            left,
            top,
            width: (right - left).max(1),
            height: (bottom - top).max(1),
        }
    }

    fn calibrate_to_cursor(&mut self) {
        if let Some(pet) = self.pet_reader.read_pet_frame() {
            let cursor = cursor_pos();
            let base = self.transform_without_offset(pet);
            self.config.offset_x = cursor.x - (base.left + base.width / 2);
            self.config.offset_y = cursor.y - (base.top + base.height / 2);
            save_setting_i32("offset-x", self.config.offset_x);
            save_setting_i32("offset-y", self.config.offset_y);
            self.update_frame(false);
        }
    }

    fn nudge(&mut self, dx: i32, dy: i32) {
        self.config.offset_x += dx;
        self.config.offset_y += dy;
        save_setting_i32("offset-x", self.config.offset_x);
        save_setting_i32("offset-y", self.config.offset_y);
        self.update_frame(false);
    }

    fn reset_offset(&mut self) {
        self.config.offset_x = 0;
        self.config.offset_y = 0;
        save_setting_i32("offset-x", 0);
        save_setting_i32("offset-y", 0);
        self.update_frame(false);
    }

    fn show_menu(&mut self) {
        unsafe {
            let menu = CreatePopupMenu().unwrap_or_default();
            let summary = to_wide(&self.summary);
            let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(summary.as_ptr()));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            append_menu_item(menu, CMD_SHOW, "Show Rings", self.rings_visible);
            append_menu_item(
                menu,
                CMD_FALLBACK,
                "Show When Pet Is Missing",
                self.config.show_without_pet,
            );
            let _ = AppendMenuW(menu, MF_STRING, CMD_REFRESH, w!("Refresh Now"));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, CMD_LEFT, w!("Left 5px"));
            let _ = AppendMenuW(menu, MF_STRING, CMD_RIGHT, w!("Right 5px"));
            let _ = AppendMenuW(menu, MF_STRING, CMD_UP, w!("Up 5px"));
            let _ = AppendMenuW(menu, MF_STRING, CMD_DOWN, w!("Down 5px"));
            let _ = AppendMenuW(menu, MF_STRING, CMD_RESET, w!("Reset Offset"));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, w!("Quit Codex Pet Limit Rings"));

            let point = cursor_pos();
            let _ = SetForegroundWindow(self.hwnd);
            let _ = TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                point.x,
                point.y,
                0,
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(menu);
        }
    }

    fn command(&mut self, id: usize) {
        match id {
            CMD_SHOW => {
                self.rings_visible = !self.rings_visible;
                self.update_frame(false);
            }
            CMD_FALLBACK => {
                self.config.show_without_pet = !self.config.show_without_pet;
                self.update_frame(true);
            }
            CMD_REFRESH => {
                self.refresh_state_async();
            }
            CMD_LEFT => self.nudge(-5, 0),
            CMD_RIGHT => self.nudge(5, 0),
            CMD_UP => self.nudge(0, -5),
            CMD_DOWN => self.nudge(0, 5),
            CMD_RESET => self.reset_offset(),
            CMD_QUIT => self.shutdown(),
            _ => {}
        }
    }

    fn shutdown(&mut self) {
        if self.disposed {
            return;
        }
        self.disposed = true;
        unsafe {
            self.remove_tray_icon();
            let _ = UnregisterHotKey(self.hwnd, HOTKEY_ALIGN);
            let _ = KillTimer(self.hwnd, TIMER_FRAME);
            let _ = KillTimer(self.hwnd, TIMER_STATE);
            let _ = KillTimer(self.hwnd, TIMER_ANIMATION);
            let _ = KillTimer(self.hwnd, TIMER_TRIM);
            PostQuitMessage(0);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        let _create = lparam.0 as *const CREATESTRUCTW;
        return LRESULT(0);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if !ptr.is_null() {
        let app = &mut *ptr;
        match msg {
            WM_TIMER => {
                match wparam.0 {
                    TIMER_FRAME => app.update_frame(false),
                    TIMER_STATE => app.refresh_state_async(),
                    TIMER_ANIMATION => {
                        app.poll_state();
                        if app.should_animate() {
                            app.phase = app.start.elapsed().as_secs_f64() / 4.6;
                            app.render();
                        }
                    }
                    TIMER_TRIM => {
                        let _ = KillTimer(hwnd, TIMER_TRIM);
                        trim_working_set();
                    }
                    _ => {}
                }
                return LRESULT(0);
            }
            WM_TRAY => {
                if lparam.0 as u32 == WM_RBUTTONUP {
                    app.show_menu();
                    return LRESULT(0);
                }
            }
            WM_COMMAND => {
                app.command(wparam.0 & 0xffff);
                return LRESULT(0);
            }
            WM_HOTKEY => {
                if wparam.0 as i32 == HOTKEY_ALIGN {
                    app.calibrate_to_cursor();
                    return LRESULT(0);
                }
            }
            WM_DESTROY => {
                app.shutdown();
                let _ = Box::from_raw(ptr);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                return LRESULT(0);
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn append_menu_item(menu: HMENU, id: usize, text: &str, checked: bool) {
    let wide = to_wide(text);
    unsafe {
        let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
        let _ = AppendMenuW(menu, flags, id, PCWSTR(wide.as_ptr()));
    }
}

fn render_rings(
    width: u32,
    height: u32,
    state: &LimitState,
    phase: f64,
    show_readout: bool,
    visual_scale: f32,
    options: &VisualOptions,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("pixmap");
    let center = (width as f32 / 2.0, height as f32 / 2.0);
    let min_side = width.min(height) as f32;
    let visual_scale = visual_scale.clamp(1.0, 2.2);
    let urgency = urgency(state.primary).max(urgency(state.secondary));
    let breathe = ((phase * 2.0 * std::f64::consts::PI).sin() as f32 + 1.0) * 0.5;
    let outer_radius = min_side * 0.5 - 16.0 * visual_scale;
    let inner_radius = outer_radius - 13.0 * visual_scale;

    draw_halo(
        pixmap.as_mut(),
        center,
        outer_radius,
        urgency,
        breathe,
        visual_scale,
    );
    draw_ticks(
        pixmap.as_mut(),
        center,
        outer_radius + 5.0 * visual_scale,
        visual_scale,
    );
    if let Some(primary) = state.primary {
        draw_ring(
            pixmap.as_mut(),
            center,
            outer_radius,
            7.0 * visual_scale,
            primary,
            options.color_for(primary.remaining_percent(), true),
            0.20,
            phase,
            visual_scale,
        );
    } else {
        draw_missing_ring(pixmap.as_mut(), center, outer_radius, 7.0 * visual_scale);
    }
    if let Some(secondary) = state.secondary {
        draw_ring(
            pixmap.as_mut(),
            center,
            inner_radius,
            4.5 * visual_scale,
            secondary,
            options.color_for(secondary.remaining_percent(), false),
            0.14,
            phase + 0.18,
            visual_scale,
        );
    }
    if show_readout {
        draw_readouts(
            pixmap.as_mut(),
            center,
            outer_radius,
            inner_radius,
            state,
            options,
            visual_scale,
        );
    }
    pixmap
}

fn urgency(bucket: Option<LimitBucket>) -> f32 {
    bucket
        .map(|bucket| ((45.0 - bucket.remaining_percent()) / 45.0).clamp(0.0, 1.0) as f32)
        .unwrap_or(0.0)
}

fn draw_halo(
    mut pixmap: PixmapMut<'_>,
    center: (f32, f32),
    radius: f32,
    urgency: f32,
    breathe: f32,
    visual_scale: f32,
) {
    let r = ((0.23 + urgency * 0.55) * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = ((0.85 - urgency * 0.30) * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = ((0.78 - urgency * 0.48) * 255.0).round().clamp(0.0, 255.0) as u8;
    let alpha = (36.0 + urgency * breathe * 24.0).round().clamp(0.0, 255.0) as u8;
    stroke_circle(
        &mut pixmap,
        center,
        radius + 3.0 * visual_scale,
        15.0 * visual_scale,
        Rgba::new(r, g, b, alpha),
    );
    stroke_circle(
        &mut pixmap,
        center,
        radius + 3.0 * visual_scale,
        8.0 * visual_scale,
        Rgba::new(r, g, b, 52 + (urgency * 22.0) as u8),
    );
    stroke_circle(
        &mut pixmap,
        center,
        radius + 13.0 * visual_scale,
        1.0 * visual_scale,
        Rgba::new(255, 255, 255, 12),
    );
}

fn draw_ticks(mut pixmap: PixmapMut<'_>, center: (f32, f32), radius: f32, visual_scale: f32) {
    for i in (0..24).step_by(2) {
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 / 24.0 * std::f32::consts::TAU;
        let inner = point(center, radius - 1.5 * visual_scale, angle);
        let outer = point(center, radius + 2.5 * visual_scale, angle);
        stroke_line(
            &mut pixmap,
            inner,
            outer,
            1.2 * visual_scale,
            Rgba::new(255, 255, 255, 26),
        );
    }
}

fn draw_ring(
    mut pixmap: PixmapMut<'_>,
    center: (f32, f32),
    radius: f32,
    width: f32,
    bucket: LimitBucket,
    color: Rgba,
    track_alpha: f32,
    phase: f64,
    visual_scale: f32,
) {
    let remaining = (bucket.remaining_percent() / 100.0).max(0.018);
    let sweep = remaining as f32 * std::f32::consts::TAU;
    let used_sweep = std::f32::consts::TAU - sweep;
    stroke_circle(
        &mut pixmap,
        center,
        radius,
        width,
        Rgba::new(255, 255, 255, (track_alpha * 255.0) as u8),
    );
    if used_sweep > 0.01 {
        let used = used_color(color);
        stroke_arc(
            &mut pixmap,
            center,
            radius,
            -std::f32::consts::FRAC_PI_2 + sweep,
            used_sweep,
            used_width(width),
            used,
        );
    }
    stroke_arc(
        &mut pixmap,
        center,
        radius,
        -std::f32::consts::FRAC_PI_2,
        sweep,
        width + 6.0 * visual_scale,
        color.with_alpha(76),
    );
    stroke_arc(
        &mut pixmap,
        center,
        radius,
        -std::f32::consts::FRAC_PI_2,
        sweep,
        width,
        color,
    );
    let glint_angle =
        -std::f32::consts::FRAC_PI_2 + ((phase - phase.floor()) as f32) * std::f32::consts::TAU;
    fill_circle(
        &mut pixmap,
        point(center, radius, glint_angle),
        1.8 * visual_scale,
        Rgba::new(255, 255, 255, 96),
    );
}

fn draw_missing_ring(mut pixmap: PixmapMut<'_>, center: (f32, f32), radius: f32, width: f32) {
    stroke_arc(
        &mut pixmap,
        center,
        radius,
        0.0,
        std::f32::consts::TAU * 0.87,
        width,
        Rgba::new(255, 255, 255, 42),
    );
}

fn draw_readouts(
    mut pixmap: PixmapMut<'_>,
    center: (f32, f32),
    outer_radius: f32,
    inner_radius: f32,
    state: &LimitState,
    options: &VisualOptions,
    visual_scale: f32,
) {
    if let Some(primary) = state.primary {
        draw_readout(
            &mut pixmap,
            center,
            outer_radius,
            primary.remaining_percent(),
            options.color_for(primary.remaining_percent(), true),
            visual_scale,
        );
    }
    if let Some(secondary) = state.secondary {
        draw_readout(
            &mut pixmap,
            center,
            inner_radius,
            secondary.remaining_percent(),
            options.color_for(secondary.remaining_percent(), false),
            visual_scale,
        );
    }
}

fn draw_readout(
    pixmap: &mut PixmapMut<'_>,
    center: (f32, f32),
    radius: f32,
    remaining: f64,
    color: Rgba,
    visual_scale: f32,
) {
    let min_side = pixmap.width().min(pixmap.height()) as f32;
    let angle =
        -std::f32::consts::FRAC_PI_2 + (remaining.max(1.8) as f32 / 100.0) * std::f32::consts::TAU;
    let label_offset = (min_side * 0.105).clamp(18.0 * visual_scale, 30.0 * visual_scale);
    let label = point(center, radius + label_offset, angle);
    let text = format!("{:.0}%", remaining.round());
    let font_px = readout_font_px(min_side, visual_scale);
    let (w, h) = readout_badge_size(&text, font_px, visual_scale);
    let edge = 3.0 * visual_scale;
    let max_x = (pixmap.width() as f32 - w - edge).max(edge);
    let max_y = (pixmap.height() as f32 - h - edge).max(edge);
    let x = (label.0 - w / 2.0).clamp(edge, max_x).round();
    let y = (label.1 - h / 2.0).clamp(edge, max_y).round();
    let radius = (h * 0.34).clamp(5.0 * visual_scale, 8.0 * visual_scale);
    fill_round_rect(pixmap, x, y, w, h, radius, Rgba::new(10, 14, 20, 184));
    stroke_round_rect(
        pixmap,
        x,
        y,
        w,
        h,
        radius,
        1.0 * visual_scale,
        color.with_alpha(180),
    );
    draw_native_label_text(
        pixmap,
        &text,
        x,
        y,
        w,
        h,
        font_px,
        Rgba::new(255, 255, 255, 238),
    );
}

fn stroke_circle(
    pixmap: &mut PixmapMut<'_>,
    center: (f32, f32),
    radius: f32,
    width: f32,
    color: Rgba,
) {
    let mut pb = PathBuilder::new();
    pb.push_circle(center.0, center.1, radius);
    if let Some(path) = pb.finish() {
        stroke_path(pixmap, &path, width, color);
    }
}

fn stroke_line(
    pixmap: &mut PixmapMut<'_>,
    from: (f32, f32),
    to: (f32, f32),
    width: f32,
    color: Rgba,
) {
    let mut pb = PathBuilder::new();
    pb.move_to(from.0, from.1);
    pb.line_to(to.0, to.1);
    if let Some(path) = pb.finish() {
        stroke_path(pixmap, &path, width, color);
    }
}

fn stroke_arc(
    pixmap: &mut PixmapMut<'_>,
    center: (f32, f32),
    radius: f32,
    start: f32,
    sweep: f32,
    width: f32,
    color: Rgba,
) {
    let segments = ((sweep.abs() / std::f32::consts::TAU) * 120.0)
        .ceil()
        .max(3.0) as usize;
    let mut pb = PathBuilder::new();
    for i in 0..=segments {
        let angle = start + sweep * i as f32 / segments as f32;
        let p = point(center, radius, angle);
        if i == 0 {
            pb.move_to(p.0, p.1);
        } else {
            pb.line_to(p.0, p.1);
        }
    }
    if let Some(path) = pb.finish() {
        stroke_path(pixmap, &path, width, color);
    }
}

fn stroke_path(pixmap: &mut PixmapMut<'_>, path: &tiny_skia::Path, width: f32, color: Rgba) {
    let mut paint = Paint::default();
    paint.set_color(color.to_skia());
    let stroke = Stroke {
        width,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    pixmap.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn fill_circle(pixmap: &mut PixmapMut<'_>, center: (f32, f32), radius: f32, color: Rgba) {
    let mut pb = PathBuilder::new();
    pb.push_circle(center.0, center.1, radius);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color.to_skia());
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn fill_round_rect(
    pixmap: &mut PixmapMut<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: Rgba,
) {
    let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(color.to_skia());
    let path = rounded_rect_path(rect, radius);
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn stroke_round_rect(
    pixmap: &mut PixmapMut<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    width: f32,
    color: Rgba,
) {
    let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) else {
        return;
    };
    let path = rounded_rect_path(rect, radius);
    stroke_path(pixmap, &path, width, color);
}

fn rounded_rect_path(rect: tiny_skia::Rect, radius: f32) -> tiny_skia::Path {
    let x = rect.left();
    let y = rect.top();
    let w = rect.width();
    let h = rect.height();
    let r = radius.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    add_arc_points(
        &mut pb,
        (x + w - r, y + r),
        r,
        -std::f32::consts::FRAC_PI_2,
        0.0,
        false,
    );
    pb.line_to(x + w, y + h - r);
    add_arc_points(
        &mut pb,
        (x + w - r, y + h - r),
        r,
        0.0,
        std::f32::consts::FRAC_PI_2,
        false,
    );
    pb.line_to(x + r, y + h);
    add_arc_points(
        &mut pb,
        (x + r, y + h - r),
        r,
        std::f32::consts::FRAC_PI_2,
        std::f32::consts::PI,
        false,
    );
    pb.line_to(x, y + r);
    add_arc_points(
        &mut pb,
        (x + r, y + r),
        r,
        std::f32::consts::PI,
        std::f32::consts::PI * 1.5,
        false,
    );
    pb.close();
    pb.finish().unwrap_or_else(|| PathBuilder::from_rect(rect))
}

fn add_arc_points(
    pb: &mut PathBuilder,
    center: (f32, f32),
    radius: f32,
    start: f32,
    end: f32,
    include_start: bool,
) {
    let steps = 8;
    for i in 0..=steps {
        if i == 0 && !include_start {
            continue;
        }
        let t = i as f32 / steps as f32;
        let angle = start + (end - start) * t;
        let p = point(center, radius, angle);
        pb.line_to(p.0, p.1);
    }
}

fn readout_font_px(min_side: f32, visual_scale: f32) -> i32 {
    (min_side * 0.078)
        .round()
        .clamp(12.0 * visual_scale, 21.0 * visual_scale) as i32
}

fn readout_badge_size(text: &str, font_px: i32, visual_scale: f32) -> (f32, f32) {
    let font = font_px as f32;
    let pad_x = (font * 0.50).round().max(6.0 * visual_scale);
    let pad_y = (font * 0.28).round().max(4.0 * visual_scale);
    let width = (text.chars().count() as f32 * font * 0.58 + pad_x * 2.0)
        .round()
        .max(34.0 * visual_scale);
    let height = (font + pad_y * 2.0).round().max(20.0 * visual_scale);
    (width, height)
}

fn draw_native_label_text(
    pixmap: &mut PixmapMut<'_>,
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    font_px: i32,
    color: Rgba,
) {
    let width = w.round().max(1.0) as i32;
    let height = h.round().max(1.0) as i32;
    let Some(mask) = render_text_mask(text, width, height, font_px) else {
        return;
    };
    blend_text_mask(
        pixmap,
        &mask,
        width as usize,
        height as usize,
        x.round() as i32,
        y.round() as i32,
        color,
    );
}

fn render_text_mask(text: &str, width: i32, height: i32, font_px: i32) -> Option<Vec<u8>> {
    if text.is_empty() || width <= 0 || height <= 0 {
        return None;
    }
    unsafe {
        let screen_dc = windows::Win32::Graphics::Gdi::GetDC(None);
        let memory_dc = CreateCompatibleDC(screen_dc);
        if memory_dc.0.is_null() {
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
            return None;
        }
        let mut bits: *mut c_void = null_mut();
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let bitmap = CreateDIBSection(
            screen_dc,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            HANDLE(null_mut()),
            0,
        );
        let Ok(bitmap) = bitmap else {
            let _ = DeleteDC(memory_dc);
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
            return None;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(memory_dc);
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
            return None;
        }
        let byte_count = width as usize * height as usize * 4;
        std::ptr::write_bytes(bits as *mut u8, 0, byte_count);

        let face = to_wide("Segoe UI");
        let font = CreateFontW(
            -font_px,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            ANTIALIASED_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_SWISS.0) as u32,
            PCWSTR(face.as_ptr()),
        );
        if font.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(memory_dc);
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
            return None;
        }

        let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let old_font = SelectObject(memory_dc, HGDIOBJ(font.0));
        let _ = SetBkMode(memory_dc, TRANSPARENT);
        let _ = SetTextColor(memory_dc, COLORREF(0x00ff_ffff));
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let _ = DrawTextW(
            memory_dc,
            &mut wide,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        let mask = std::slice::from_raw_parts(bits as *const u8, byte_count).to_vec();
        let _ = SelectObject(memory_dc, old_font);
        let _ = SelectObject(memory_dc, old_bitmap);
        let _ = DeleteObject(HGDIOBJ(font.0));
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
        Some(mask)
    }
}

fn blend_text_mask(
    pixmap: &mut PixmapMut<'_>,
    mask: &[u8],
    mask_width: usize,
    mask_height: usize,
    dst_x: i32,
    dst_y: i32,
    color: Rgba,
) {
    let pix_width = pixmap.width() as i32;
    let pix_height = pixmap.height() as i32;
    let stride = pixmap.width() as usize * 4;
    let data = pixmap.data_mut();
    for y in 0..mask_height {
        let py = dst_y + y as i32;
        if py < 0 || py >= pix_height {
            continue;
        }
        for x in 0..mask_width {
            let px = dst_x + x as i32;
            if px < 0 || px >= pix_width {
                continue;
            }
            let mask_index = (y * mask_width + x) * 4;
            let coverage = mask[mask_index]
                .max(mask[mask_index + 1])
                .max(mask[mask_index + 2]);
            if coverage == 0 {
                continue;
            }
            let src_a = (coverage as u32 * color.a as u32 + 127) / 255;
            if src_a == 0 {
                continue;
            }
            let inv_a = 255 - src_a;
            let dst_index = py as usize * stride + px as usize * 4;
            data[dst_index] =
                ((color.r as u32 * src_a + data[dst_index] as u32 * inv_a) / 255) as u8;
            data[dst_index + 1] =
                ((color.g as u32 * src_a + data[dst_index + 1] as u32 * inv_a) / 255) as u8;
            data[dst_index + 2] =
                ((color.b as u32 * src_a + data[dst_index + 2] as u32 * inv_a) / 255) as u8;
            data[dst_index + 3] = (src_a + data[dst_index + 3] as u32 * inv_a / 255) as u8;
        }
    }
}

fn point(center: (f32, f32), radius: f32, angle: f32) -> (f32, f32) {
    (
        center.0 + angle.cos() * radius,
        center.1 + angle.sin() * radius,
    )
}

fn used_color(color: Rgba) -> Rgba {
    Rgba::new(
        blend(color.r, 255, 0.74),
        blend(color.g, 255, 0.74),
        blend(color.b, 255, 0.74),
        92,
    )
}

fn used_width(width: f32) -> f32 {
    (width * 0.55).max(2.0)
}

fn blend(from: u8, to: u8, amount: f64) -> u8 {
    (from as f64 + (to as f64 - from as f64) * amount).round() as u8
}

fn rgba_to_bgra_premul(data: &mut [u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        out.push(chunk[2]);
        out.push(chunk[1]);
        out.push(chunk[0]);
        out.push(chunk[3]);
    }
    out
}

unsafe fn update_layered(hwnd: HWND, bounds: Rect, bgra: &[u8]) {
    let screen_dc = windows::Win32::Graphics::Gdi::GetDC(None);
    let memory_dc = CreateCompatibleDC(screen_dc);
    let mut bits: *mut c_void = null_mut();
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: bounds.width,
            biHeight: -bounds.height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let bitmap = CreateDIBSection(
        screen_dc,
        &info,
        DIB_RGB_COLORS,
        &mut bits,
        HANDLE(null_mut()),
        0,
    );
    if let Ok(bitmap) = bitmap {
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(memory_dc);
            let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
            return;
        }
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
        let old = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let size = SIZE {
            cx: bounds.width,
            cy: bounds.height,
        };
        let src = POINT { x: 0, y: 0 };
        let dst = POINT {
            x: bounds.left,
            y: bounds.top,
        };
        let blend = BLENDFUNCTION {
            BlendOp: 0,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: 1,
        };
        let _ = windows::Win32::UI::WindowsAndMessaging::UpdateLayeredWindow(
            hwnd,
            screen_dc,
            Some(&dst),
            Some(&size),
            memory_dc,
            Some(&src),
            COLORREF(0),
            Some(&blend),
            windows::Win32::UI::WindowsAndMessaging::ULW_ALPHA,
        );
        let _ = SelectObject(memory_dc, old);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
    }
    let _ = DeleteDC(memory_dc);
    let _ = windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);
}

fn is_mouse_over(pet: Option<Rect>, ring: Rect) -> bool {
    let mouse = cursor_pos();
    if pet.map(|pet| pet.contains(mouse, 10)).unwrap_or(false) {
        return true;
    }
    if !ring.contains(mouse, 4) {
        return false;
    }
    let local_x = mouse.x - ring.left;
    let local_y = mouse.y - ring.top;
    let center = ring.width as f64 / 2.0;
    let dx = local_x as f64 - center;
    let dy = local_y as f64 - center;
    let distance = (dx * dx + dy * dy).sqrt();
    let visual_scale = visual_scale_for_rect(ring) as f64;
    let radius = ring.width.min(ring.height) as f64 * 0.5 - 16.0 * visual_scale;
    distance >= radius - 24.0 * visual_scale && distance <= radius + 19.0 * visual_scale
}

fn pet_frame_from_live_overlay(
    mascot: Rect,
    state_overlay: Rect,
    live_overlay: Rect,
    offset_x: i32,
    offset_y: i32,
    manual_scale_x: f64,
    manual_scale_y: f64,
) -> Rect {
    if state_overlay.width <= 0 || state_overlay.height <= 0 {
        return Rect {
            left: live_overlay.left + offset_x,
            top: live_overlay.top + offset_y,
            width: live_overlay.width,
            height: live_overlay.height,
        };
    }
    let live_scale = (live_overlay.width as f64 / state_overlay.width as f64).clamp(0.45, 2.6);
    let scale_x = live_scale * manual_scale_x;
    let scale_y = live_scale * manual_scale_y;
    Rect {
        left: live_overlay.left + (mascot.left as f64 * scale_x).round() as i32 + offset_x,
        top: live_overlay.top + (mascot.top as f64 * scale_y).round() as i32 + offset_y,
        width: (mascot.width as f64 * scale_x).round().max(1.0) as i32,
        height: (mascot.height as f64 * scale_y).round().max(1.0) as i32,
    }
}

fn left_button_down() -> bool {
    unsafe { (GetAsyncKeyState(0x01) as u16 & 0x8000) != 0 }
}

fn fallback_pet_frame(size: i32) -> Rect {
    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let pet_size = (size - 76).max(96);
        Rect {
            left: screen_w - pet_size - 48,
            top: screen_h - pet_size - 48,
            width: pet_size,
            height: pet_size,
        }
    }
}

fn cursor_pos() -> POINT {
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    point
}

fn format_percent(percent: f64) -> String {
    if (percent.round() - percent).abs() < 0.05 {
        format!("{}%", percent.round() as i32)
    } else {
        format!("{percent:.1}%")
    }
}

fn parse_config() -> Option<Config> {
    let home = env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(PathBuf::from))?;
    let mut codex_home = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let mut state = codex_home.join(".codex-global-state.json");
    let mut logs = if codex_home.join("logs_2.sqlite").exists() {
        codex_home.join("logs_2.sqlite")
    } else {
        codex_home.join("logs_1.sqlite")
    };
    let mut auth = codex_home.join("auth.json");
    let mut preview = None;
    let mut size = 220;
    let mut show_without_pet = false;
    let mut offset_x = load_setting_i32("offset-x", 0);
    let mut offset_y = load_setting_i32("offset-y", 0);
    let mut scale_x = 1.0;
    let mut scale_y = 1.0;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return None,
            "--preview" => preview = args.next().map(PathBuf::from),
            "--codex-home" => {
                codex_home = args.next().map(PathBuf::from)?;
                state = codex_home.join(".codex-global-state.json");
                logs = if codex_home.join("logs_2.sqlite").exists() {
                    codex_home.join("logs_2.sqlite")
                } else {
                    codex_home.join("logs_1.sqlite")
                };
                auth = codex_home.join("auth.json");
            }
            "--logs" => logs = args.next().map(PathBuf::from)?,
            "--auth" => auth = args.next().map(PathBuf::from)?,
            "--state" => state = args.next().map(PathBuf::from)?,
            "--size" => size = args.next()?.parse().ok()?,
            "--show-without-pet" => show_without_pet = true,
            "--offset-x" => offset_x = args.next()?.parse().ok()?,
            "--offset-y" => offset_y = args.next()?.parse().ok()?,
            "--scale-x" => scale_x = args.next()?.parse().ok()?,
            "--scale-y" => scale_y = args.next()?.parse().ok()?,
            _ => return None,
        }
    }

    Some(Config {
        state_path: state,
        logs_path: logs,
        auth_path: auth,
        preview_path: preview,
        size,
        show_without_pet,
        offset_x,
        offset_y,
        scale_x,
        scale_y,
        visual: VisualOptions::default(),
    })
}

fn settings_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("CodexPetLimitRings")
}

fn load_setting_i32(name: &str, fallback: i32) -> i32 {
    fs::read_to_string(settings_dir().join(format!("{name}.txt")))
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(fallback)
}

fn save_setting_i32(name: &str, value: i32) {
    let dir = settings_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(format!("{name}.txt")), value.to_string());
}

fn visual_scale_for_rect(rect: Rect) -> f32 {
    let (scale_x, scale_y) = monitor_scale_for_rect(rect);
    (((scale_x + scale_y) * 0.5) as f32).clamp(1.0, 2.2)
}

fn scaled_i32(value: i32, scale: f32) -> i32 {
    ((value as f32 * scale).round() as i32).max(1)
}

fn monitor_scale_for_rect(rect: Rect) -> (f64, f64) {
    let (x, y) = rect.center();
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        if !monitor.0.is_null() {
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
                return (
                    (dpi_x as f64 / 96.0).clamp(0.75, 4.0),
                    (dpi_y as f64 / 96.0).clamp(0.75, 4.0),
                );
            }
        }
    }
    let scale = system_scale();
    (scale, scale)
}

fn system_scale() -> f64 {
    unsafe {
        let dpi = windows::Win32::UI::HiDpi::GetDpiForSystem();
        (dpi as f64 / 96.0).clamp(0.75, 4.0)
    }
}

fn enable_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT((-4isize) as *mut c_void));
    }
}

fn trim_working_set() {
    unsafe {
        let _ = EmptyWorkingSet(GetCurrentProcess());
    }
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn write_wide_fixed<const N: usize>(target: &mut [u16; N], text: &str) {
    target.fill(0);
    for (slot, ch) in target
        .iter_mut()
        .take(N.saturating_sub(1))
        .zip(text.encode_utf16())
    {
        *slot = ch;
    }
}

fn render_preview(config: &Config) -> bool {
    let Some(path) = &config.preview_path else {
        return false;
    };
    let state = StateReader {
        logs: config.logs_path.clone(),
        auth: config.auth_path.clone(),
    }
    .read_latest();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let pixmap = render_rings(
        config.size as u32,
        config.size as u32,
        &state,
        0.18,
        true,
        1.0,
        &config.visual,
    );
    pixmap.save_png(path).is_ok()
}

fn main() {
    let Some(config) = parse_config() else {
        return;
    };
    if config.preview_path.is_some() {
        let _ = render_preview(&config);
        return;
    }
    let _ = App::new(config).run();
}
