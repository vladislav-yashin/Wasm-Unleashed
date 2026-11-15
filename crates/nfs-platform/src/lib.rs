//! Host services shared by parsers, assets, audio and game code.
#[cfg(not(target_arch = "wasm32"))]
pub use std::fs;
#[cfg(target_arch = "wasm32")]
pub mod fs;
/// The browser's WebSocket bridge to the UDP relay (`packaging/web/net.mjs`).
#[cfg(target_arch = "wasm32")]
pub mod net;
#[cfg(not(target_arch = "wasm32"))]
pub use std::time;
#[cfg(target_arch = "wasm32")]
pub use web_time as time;

/// Path predicates must consult the same source as reads on the web.
pub trait PathExt {
    fn host_exists(&self) -> bool;
    fn host_is_file(&self) -> bool;
    fn host_is_dir(&self) -> bool;
}
impl PathExt for std::path::Path {
    fn host_exists(&self) -> bool { fs::metadata(self).is_ok() }
    fn host_is_file(&self) -> bool { fs::metadata(self).is_ok_and(|m| m.is_file()) }
    fn host_is_dir(&self) -> bool { fs::metadata(self).is_ok_and(|m| m.is_dir()) }
}

#[cfg(not(target_arch = "wasm32"))]
pub use std::thread;
#[cfg(target_arch = "wasm32")]
pub mod thread {
    pub use std::thread::available_parallelism;
    pub struct Scope;
    pub struct ScopedJoinHandle<'a, T>(T, std::marker::PhantomData<&'a ()>);
    impl Scope {
        pub fn spawn<'a, F, T>(&'a self, f: F) -> ScopedJoinHandle<'a, T> where F: FnOnce() -> T {
            ScopedJoinHandle(f(), std::marker::PhantomData)
        }
    }
    impl<T> ScopedJoinHandle<'_, T> {
        pub fn join(self) -> std::thread::Result<T> { Ok(self.0) }
    }
    pub fn scope<F, T>(f: F) -> T where F: FnOnce(&Scope) -> T { f(&Scope) }
}

#[cfg(target_arch = "wasm32")]
pub async fn delay(ms: u32) {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(inline_js = "export function nfsDelay(ms){return new Promise(resolve=>setTimeout(resolve,ms))}")]
    extern "C" { fn nfsDelay(ms: u32) -> js_sys::Promise; }
    let _ = wasm_bindgen_futures::JsFuture::from(nfsDelay(ms)).await;
}

/// Fetch a range of a segmented, immutable browser archive before synchronous parsing.
#[cfg(target_arch = "wasm32")]
pub async fn read_range_async(path: &str, offset: u32, size: u32) -> std::io::Result<Vec<u8>> {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(inline_js = "export function nfsReadRangeAsync(path,offset,size){return globalThis.nfsVfs.readRangeAsync(path,offset,size)}")]
    extern "C" { fn nfsReadRangeAsync(path: &str, offset: u32, size: u32) -> js_sys::Promise; }
    let bytes = wasm_bindgen_futures::JsFuture::from(nfsReadRangeAsync(path, offset, size)).await.map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    Ok(js_sys::Uint8Array::new(&bytes).to_vec())
}

#[cfg(target_arch = "wasm32")]
pub fn browser_failure(message: &str) {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(inline_js = "export function nfsBrowserFailure(message){globalThis.nfsFatal?.(message)}")]
    extern "C" { fn nfsBrowserFailure(message: &str); }
    nfsBrowserFailure(message);
}
