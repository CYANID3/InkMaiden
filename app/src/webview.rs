//! Один exe без WebView2Loader.dll рядом: загрузчик WebView2 (крошечная DLL от Microsoft) встроен в программу.
//! При запуске он кладётся в %LOCALAPPDATA%\InkMaiden\runtime и загружается до создания окна;
//! сама программа подключает его «отложенно» (см. build-support/webview2-com-sys/build.rs), поэтому без файла exe всё равно стартует.

#[cfg(windows)]
pub fn prepare() {
    use std::os::windows::ffi::OsStrExt;
    const DLL: &[u8] = include_bytes!("../../build-support/webview2-com-sys/x64/WebView2Loader.dll");
    extern "system" { fn LoadLibraryW(name: *const u16) -> *mut core::ffi::c_void; }
    let base = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let dir = base.join("InkMaiden").join("runtime");
    let path = dir.join("WebView2Loader.dll");
    if std::fs::metadata(&path).map(|m| m.len() != DLL.len() as u64).unwrap_or(true) {
        let _ = std::fs::create_dir_all(&dir);
        let tmp = dir.join(format!("WebView2Loader.{}.tmp", std::process::id()));
        if std::fs::write(&tmp, DLL).is_ok() { let _ = std::fs::rename(&tmp, &path); }
    }
    let w: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe { LoadLibraryW(w.as_ptr()); }
}

#[cfg(not(windows))]
pub fn prepare() {}
