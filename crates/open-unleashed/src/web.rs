//! Browser lifecycle; the shell prepares startup resources before WASM and deferred assets before decoding.
use crate::*;
use std::cell::{Cell, RefCell};
use winit::platform::web::{WindowAttributesExtWebSys, WindowExtWebSys};
thread_local! {
    static STARTING: Cell<bool> = const { Cell::new(false) };
    static READY: RefCell<Option<(Arc<Window>, Renderer)>> = const { RefCell::new(None) };
}
pub fn start() {
    let _ = tracing_subscriber::fmt().without_time().with_ansi(false).with_max_level(tracing::Level::INFO).with_writer(ConsoleWriter::default).try_init();
    std::panic::set_hook(Box::new(|panic| web_sys::console::error_1(&format!("{panic}").into())));
    // The shell's launch setting, before anything reads the save directory: an empty value (or the
    // smoke page) keeps the default name.
    let unlock_name = unlock_profile_name();
    if !unlock_name.trim().is_empty() && !nfs_game::save::set_unlock_all_name(&unlock_name) {
        tracing::warn!(name = %unlock_name, "ignoring an invalid unlock-all profile name");
    }
    if nfsSmoke() { smoke(); return; }
    if let Err(error) = run() { wasm_bindgen::throw_str(&format!("{error:#}")); }
}
pub fn renderer_ready() -> bool { READY.with(|r| r.borrow().is_some()) }
pub fn take_renderer() -> Option<(Arc<Window>, Renderer)> { READY.with(|r| r.borrow_mut().take()) }
pub fn begin_renderer(event_loop: &ActiveEventLoop) {
    if STARTING.with(|s| s.replace(true)) { return; }
    let attrs = Window::default_attributes().with_title(crate::brand::TITLE).with_append(true);
    let window = Arc::new(event_loop.create_window(attrs).expect("browser canvas"));
    if let Some(canvas) = window.canvas() { canvas.set_id("game"); }
    wasm_bindgen_futures::spawn_local(async move {
        match Renderer::new_async(window.clone()).await {
            Ok(mut renderer) => {
                // the shell's render scale (`#render-scale`), applied to the first surface too
                let size = surface_size(window.inner_size());
                renderer.resize(size.width, size.height);
                READY.with(|r| *r.borrow_mut() = Some((window, renderer)));
            }
            Err(error) => nfsFatal(&format!("Graphics initialization failed: {error:#}")),
        }
    });
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsRenderScale(){const s=Number(globalThis.nfsRenderScale);return s>=0.25&&s<=1?s:1}")]
extern "C" {
    fn nfsRenderScale() -> f64;
}
/// The browser shell's **render scale** (`#render-scale`, remembered in `localStorage`): the
/// fraction of the canvas's pixels the game draws at. The canvas is still stretched over the whole
/// viewport by CSS (`style.css`), so a smaller surface is simply upscaled by the browser — a
/// platform performance setting, not a game rule. The game's own `Graphics.Resolutions` pick is the
/// window's size (and does nothing in a browser, where the canvas fills the viewport).
pub fn render_scale() -> f64 { nfsRenderScale().clamp(0.25, 1.0) }

/// The canvas-backing size for a window size, after [`render_scale`].
pub fn surface_size(size: winit::dpi::PhysicalSize<u32>) -> winit::dpi::PhysicalSize<u32> {
    let s = render_scale();
    if s >= 1.0 { return size; }
    winit::dpi::PhysicalSize::new(((size.width as f64 * s) as u32).max(1), ((size.height as f64 * s) as u32).max(1))
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsUnlockName(){return String(globalThis.nfsUnlockName || '')}")]
extern "C" {
    fn nfsUnlockName() -> String;
}
/// The browser shell's **UnlockAll name** field (`#unlock-name`, kept in `localStorage`): the name
/// the port-created profile gets. It is a launch setting and must be pinned before the game reads
/// the save directory, so [`start`] applies it before `run`.
pub fn unlock_profile_name() -> String { nfsUnlockName() }

thread_local! { static RACE_RETRY: Cell<bool> = const { Cell::new(false) }; static LOADING: Cell<bool> = const { Cell::new(false) }; }
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsTrackReady(name){return globalThis.nfsTrackReady(name)} export function nfsPrepareTrack(name){return globalThis.nfsPrepareTrack(name)} export function nfsTakeText(){const pending=globalThis.nfsPendingText;delete globalThis.nfsPendingText;return pending || null}")]
extern "C" {
    fn nfsTrackReady(name: &str) -> bool;
    fn nfsPrepareTrack(name: &str) -> js_sys::Promise;
    fn nfsTakeText() -> wasm_bindgen::JsValue;
}
pub fn take_text() -> Option<(String, String, bool)> {
    let value = nfsTakeText();
    let get = |name: &str| js_sys::Reflect::get(&value, &name.into()).ok();
    Some((get("id")?.as_string()?, get("text")?.as_string()?, get("commit")?.as_bool()?))
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsTextField(id,x,y,w,h,text,max,digits){globalThis.nfsTextField?.(id,x,y,w,h,text,max,digits)} export function nfsTouchBottom(){return matchMedia('(any-pointer:coarse)').matches && document.documentElement.dataset.controlsMode==='race' ? (Math.max(0,innerHeight-(document.querySelector('.steering')?.getBoundingClientRect().top ?? innerHeight))+8)/innerHeight : 0}")]
extern "C" {
    fn nfsTextField(id: &str, x: f32, y: f32, w: f32, h: f32, text: &str, max: usize, digits: bool);
    fn nfsTouchBottom() -> f32;
}
pub fn text_field(id: &str, rect: [f32; 4], text: &str, max: usize, digits: bool) {
    nfsTextField(id, rect[0], rect[1], rect[2], rect[3], text, max, digits);
}
pub fn touch_bottom() -> f32 { nfsTouchBottom() }

pub fn loading() -> bool { LOADING.with(Cell::get) }
pub fn take_race_retry() -> bool { RACE_RETRY.with(|r| r.replace(false)) }
/// Ask for another `start_from_layout`: the shell's download finishing and the ready frame being
/// presented both defer the launch by one frame-loop turn.
pub fn request_race_retry() { RACE_RETRY.with(|r| r.set(true)); }
pub fn prepare_track(name: &str) -> bool {
    if nfsTrackReady(name) { return true; }
    if LOADING.with(|s| s.replace(true)) { return false; }
    let name = name.to_owned();
    wasm_bindgen_futures::spawn_local(async move {
        let result = wasm_bindgen_futures::JsFuture::from(nfsPrepareTrack(&name)).await;
        LOADING.with(|s| s.set(false));
        match result {
            Ok(_) => RACE_RETRY.with(|r| r.set(true)),
            Err(error) => web_sys::console::error_1(&error),
        }
    });
    false
}

/// The shell's own track panel: `true` hides it while the game draws 0x4A4A70 itself, `false` lets
/// it take over again (its Retry / error report shows it regardless).
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsTrackScreen(active){globalThis.nfsTrackScreen?.(active)} export function nfsTrackProgress(){return globalThis.nfsTrackProgress?.() ?? 0}")]
extern "C" {
    fn nfsTrackScreen(active: bool);
    fn nfsTrackProgress() -> f64;
}
pub fn track_screen(active: bool) { nfsTrackScreen(active) }
/// The share of the launch's files the shell has prepared so far, 0…1 (`fs.preload` in
/// `packaging/web/bootstrap.mjs`).
pub fn track_progress() -> f64 { nfsTrackProgress().clamp(0.0, 1.0) }

thread_local! { static MEDIA_RETRY: Cell<bool> = const { Cell::new(false) }; }
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsFilesReady(paths){return globalThis.nfsFilesReady(paths)} export function nfsPrepareFiles(paths){return globalThis.nfsPrepareFiles(paths)}")]
extern "C" {
    fn nfsFilesReady(paths: &js_sys::Array) -> bool;
    fn nfsPrepareFiles(paths: &js_sys::Array) -> js_sys::Promise;
}

/// Resolve optional original media using the manifest's case-insensitive directory
/// view, then wait for every existing file before invoking a synchronous decoder.
/// Missing original files retain the native front end's fallback behavior.
pub fn prepare_frontend_files(paths: &GamePaths, directory: &str, names: &[String]) -> bool {
    let dir = paths.fe_data().join(directory);
    prepare_directory_files(&dir, names)
}

pub fn prepare_car_models(paths: &GamePaths, models: &[String]) -> bool {
    let names: Vec<String> = models.iter()
        .flat_map(|model| [".crp", ".tpg", "D.fsh", "L.fsh"].map(|suffix| format!("{model}{suffix}")))
        .collect();
    prepare_directory_files(&paths.car_model_dir(), &names)
}

fn prepare_directory_files(dir: &std::path::Path, names: &[String]) -> bool {
    let files: js_sys::Array = names.iter()
        .filter_map(|name| GamePaths::find_file(dir, name))
        .map(|path| wasm_bindgen::JsValue::from_str(&path.to_string_lossy()))
        .collect();
    if nfsFilesReady(&files) { return true; }
    if LOADING.with(|s| s.replace(true)) { return false; }
    wasm_bindgen_futures::spawn_local(async move {
        let result = wasm_bindgen_futures::JsFuture::from(nfsPrepareFiles(&files)).await;
        LOADING.with(|s| s.set(false));
        match result {
            Ok(_) => MEDIA_RETRY.with(|r| r.set(true)),
            Err(error) => nfsFatal(&format!("Media preparation failed: {error:?}")),
        }
    });
    false
}

pub fn take_media_retry() -> bool { MEDIA_RETRY.with(|r| r.replace(false)) }

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsFatal(message){globalThis.nfsFatal?.(message)}")]
extern "C" { fn nfsFatal(message: &str); }

/// Tell the browser shell that the player left the game through its own UI. The shell flushes the
/// saves, leaves fullscreen and replaces the page with the site's home page; a browser page cannot
/// close its own tab.
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsExitGame(){globalThis.nfsExitGame?.()}")]
extern "C" { fn nfsExitGame(); }
pub fn exit_game() { nfsExitGame(); }
thread_local! { static PRIMARY_TOUCH: Cell<Option<u64>> = const { Cell::new(None) }; }
pub fn primary_touch(touch: &winit::event::Touch) -> bool {
    PRIMARY_TOUCH.with(|id| {
        if touch.phase == winit::event::TouchPhase::Started && id.get().is_none() { id.set(Some(touch.id)); }
        if id.get() != Some(touch.id) { return false; }
        if matches!(touch.phase, winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled) { id.set(None); }
        true
    })
}
thread_local! { static REQUESTED_SIZE: Cell<Option<(u32, u32)>> = const { Cell::new(None) }; }
pub fn resize(window: &Window) {
    let Some(browser) = web_sys::window() else { return };
    let width = browser.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(960.0);
    let height = browser.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(600.0);
    let scale = browser.device_pixel_ratio().min(1.5).min((1920.0 * 1080.0 / (width * height)).sqrt());
    let size = winit::dpi::PhysicalSize::new((width * scale).max(1.0) as u32, (height * scale).max(1.0) as u32);
    // CSS controls the displayed canvas size. A request may therefore differ from
    // winit's observed size; do not rewrite its style on every event-loop tick.
    if REQUESTED_SIZE.with(|last| last.replace(Some((size.width, size.height)))) != Some((size.width, size.height)) {
        let _ = window.request_inner_size(size);
    }
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsSmoke(){return location.pathname.endsWith('/smoke.html')} export function nfsSmokeDone(label){globalThis.nfsSmokeDone?.(label)}")]
extern "C" { fn nfsSmoke() -> bool; fn nfsSmokeDone(label: &str); }
fn smoke() {
    struct Smoke;
    impl winit::application::ApplicationHandler for Smoke {
        fn window_event(&mut self, _: &ActiveEventLoop, _: winit::window::WindowId, _: winit::event::WindowEvent) {}
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let window = Arc::new(event_loop.create_window(Window::default_attributes().with_append(true)).expect("smoke canvas"));
            wasm_bindgen_futures::spawn_local(async move {
                match Renderer::new_async(window).await {
                    Ok(mut renderer) => {
                        match renderer.render(&nfs_renderer::Scene::default(), &nfs_renderer::Camera::default()) {
                            Ok(()) => nfsSmokeDone(nfs_renderer::ADAPTER_LABEL.get().map_or("GPU", String::as_str)),
                            Err(error) => nfsFatal(&format!("smoke frame: {error:#}")),
                        }
                        // Keep the surface alive through presentation.
                        nfs_platform::delay(1000).await;
                    }
                    Err(error) => nfsFatal(&format!("smoke initialization: {error:#}")),
                }
            });
        }
    }
    use winit::platform::web::EventLoopExtWebSys;
    EventLoop::new().expect("smoke event loop").spawn_app(Smoke);
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function nfsReportState(mode,paused,speed,driving){globalThis.nfsUpdateControls?.(mode,paused,driving);document.documentElement.dataset.speed=String(speed);if(document.documentElement.dataset.gameMode!==mode)document.documentElement.dataset.gameMode=mode;document.documentElement.dataset.paused=String(paused)} export function nfsDownloadFrame(){document.querySelector('canvas').toBlob(blob=>{if(!blob){globalThis.nfsFatal?.('Photo export failed');return}const url=URL.createObjectURL(blob);const link=document.createElement('a');link.href=url;link.download='WasmUnleashed.png';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000)})}")]
extern "C" { fn nfsReportState(mode: &str, paused: bool, speed: f32, driving: bool); fn nfsDownloadFrame(); }
pub fn report_state(mode: &str, paused: bool, speed: f32, driving: bool) { nfsReportState(mode, paused, speed, driving); }
pub fn download_frame() { nfsDownloadFrame(); }

#[derive(Default)]
struct ConsoleWriter(Vec<u8>);
impl std::io::Write for ConsoleWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> { self.0.extend_from_slice(bytes); Ok(bytes.len()) }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
impl Drop for ConsoleWriter {
    fn drop(&mut self) {
        let text = String::from_utf8_lossy(&self.0);
        if text.contains("ERROR") { web_sys::console::error_1(&text.as_ref().into()); }
        else if !text.is_empty() { web_sys::console::log_1(&text.as_ref().into()); }
    }
}
