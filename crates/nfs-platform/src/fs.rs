//! Synchronous view of resources preloaded by the browser, plus writable save files.
use std::{io::{self, Read, Write, Seek, SeekFrom}, path::{Path, PathBuf}};
use wasm_bindgen::prelude::*;
#[wasm_bindgen(inline_js = "export function nfsRead(p){return globalThis.nfsVfs.read(p)} export function nfsLength(p){return globalThis.nfsVfs.length(p)} export function nfsReadRange(p,start,len){return globalThis.nfsVfs.readRange(p,start,len)} export function nfsWrite(p,b){globalThis.nfsVfs.write(p,b)} export function nfsList(p){return globalThis.nfsVfs.list(p)} export function nfsRemove(p){globalThis.nfsVfs.remove(p)} export function nfsMkdir(p){globalThis.nfsVfs.mkdir(p)} export function nfsStat(p){return globalThis.nfsVfs.stat(p)}")]
extern "C" {
    #[wasm_bindgen(catch)] fn nfsLength(p: &str) -> Result<f64, JsValue>;
    #[wasm_bindgen(catch)] fn nfsReadRange(p: &str, start: f64, len: u32) -> Result<js_sys::Uint8Array, JsValue>;
    #[wasm_bindgen(catch)] fn nfsRead(p: &str) -> Result<js_sys::Uint8Array, JsValue>;
    #[wasm_bindgen(catch)] fn nfsWrite(p: &str, bytes: &[u8]) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)] fn nfsList(p: &str) -> Result<js_sys::Array, JsValue>;
    #[wasm_bindgen(catch)] fn nfsRemove(p: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)] fn nfsMkdir(p: &str) -> Result<(), JsValue>;
    fn nfsStat(p: &str) -> u8;
}
fn error(e: JsValue) -> io::Error { io::Error::other(format!("{e:?}")) }
fn name(p: impl AsRef<Path>) -> String { p.as_ref().to_string_lossy().into_owned() }
pub fn read(p: impl AsRef<Path>) -> io::Result<Vec<u8>> { nfsRead(&name(p)).map(|a| a.to_vec()).map_err(error) }
pub fn read_to_string(p: impl AsRef<Path>) -> io::Result<String> { String::from_utf8(read(p)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)) }
pub fn write(p: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> { nfsWrite(&name(p), bytes.as_ref()).map_err(error) }
pub fn create_dir_all(p: impl AsRef<Path>) -> io::Result<()> { nfsMkdir(&name(p)).map_err(error) }
pub fn remove_file(p: impl AsRef<Path>) -> io::Result<()> { nfsRemove(&name(p)).map_err(error) }
pub fn copy(a: impl AsRef<Path>, b: impl AsRef<Path>) -> io::Result<u64> { let bytes = read(a)?; write(b, &bytes)?; Ok(bytes.len() as u64) }
pub fn rename(a: impl AsRef<Path>, b: impl AsRef<Path>) -> io::Result<()> { copy(&a, b)?; remove_file(a) }
pub fn exists(p: impl AsRef<Path>) -> io::Result<bool> { Ok(nfsStat(&name(p)) != 0) }
pub struct Metadata(u8);
impl Metadata { pub fn is_file(&self) -> bool { self.0 == 1 } pub fn is_dir(&self) -> bool { self.0 == 2 } }
pub fn metadata(p: impl AsRef<Path>) -> io::Result<Metadata> { match nfsStat(&name(p)) { 0 => Err(io::ErrorKind::NotFound.into()), n => Ok(Metadata(n)) } }
pub struct DirEntry(PathBuf);
impl DirEntry { pub fn path(&self) -> PathBuf { self.0.clone() } pub fn file_name(&self) -> std::ffi::OsString { self.0.file_name().unwrap_or_default().to_owned() } pub fn file_type(&self) -> io::Result<Metadata> { metadata(&self.0) } }
pub fn read_dir(p: impl AsRef<Path>) -> io::Result<impl Iterator<Item = io::Result<DirEntry>>> {
    let root = p.as_ref();
    let list = nfsList(&name(root)).map_err(error)?;
    Ok(list.iter().filter_map(|v| v.as_string()).map(|s| Ok(DirEntry(root.join(s)))).collect::<Vec<_>>().into_iter())
}
pub struct File { path: PathBuf, data: io::Cursor<Vec<u8>>, writable: bool, length: u64 }
impl File {
    pub fn open(p: impl AsRef<Path>) -> io::Result<Self> { Ok(Self { path: p.as_ref().into(), data: io::Cursor::new(Vec::new()), writable: false, length: nfsLength(&name(p)).map_err(error)? as u64 }) }
    pub fn create(p: impl AsRef<Path>) -> io::Result<Self> { write(&p, [])?; Ok(Self { path: p.as_ref().into(), data: io::Cursor::new(Vec::new()), writable: true, length: 0 }) }
    pub fn sync_all(&self) -> io::Result<()> { if self.writable { write(&self.path, self.data.get_ref())?; } Ok(()) }
    pub fn metadata(&self) -> io::Result<FileMetadata> { Ok(FileMetadata(if self.writable { self.data.get_ref().len() as u64 } else { self.length })) }
}
pub struct FileMetadata(u64);
impl FileMetadata { pub fn len(&self) -> u64 { self.0 } pub fn is_empty(&self) -> bool { self.0 == 0 } }
impl Read for File { fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { if self.writable { return self.data.read(buf); }
        let bytes = nfsReadRange(&name(&self.path), self.data.position() as f64, buf.len().min(u32::MAX as usize) as u32).map_err(error)?;
        let n = bytes.length() as usize;
        bytes.copy_to(&mut buf[..n]);
        self.data.set_position(self.data.position() + n as u64);
        Ok(n)
    } }
impl Seek for File { fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> { if self.writable { return self.data.seek(pos); }
        let position = match pos {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(n) => self.data.position() as i128 + n as i128,
            SeekFrom::End(n) => self.length as i128 + n as i128,
        };
        if !(0..=u64::MAX as i128).contains(&position) { return Err(io::ErrorKind::InvalidInput.into()); }
        self.data.set_position(position as u64);
        Ok(position as u64)
    } }
impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { if !self.writable { return Err(io::ErrorKind::PermissionDenied.into()); } let n = self.data.write(buf)?; self.sync_all()?; Ok(n) }
    fn flush(&mut self) -> io::Result<()> { self.sync_all() }
}
#[derive(Default)]
pub struct OpenOptions { append: bool, create: bool, write: bool, truncate: bool }
impl OpenOptions {
    pub fn new() -> Self { Self::default() }
    pub fn append(&mut self, v: bool) -> &mut Self { self.append = v; self }
    pub fn create(&mut self, v: bool) -> &mut Self { self.create = v; self }
    pub fn write(&mut self, v: bool) -> &mut Self { self.write = v; self }
    pub fn truncate(&mut self, v: bool) -> &mut Self { self.truncate = v; self }
    pub fn open(&self, p: impl AsRef<Path>) -> io::Result<File> {
        let mut file = match File::open(&p) { Ok(f) => f, Err(e) if self.create => { let _ = e; File::create(&p)? }, Err(e) => return Err(e) };
        file.writable = self.write || self.append;
        if file.writable && !self.truncate { file.data = io::Cursor::new(read(&p)?); }
        if self.truncate && file.writable { file.data = io::Cursor::new(Vec::new()); file.sync_all()?; }
        if self.append { file.seek(SeekFrom::End(0))?; }
        Ok(file)
    }
}
