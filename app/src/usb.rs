//! Какие читалки PocketBook сейчас подключены к компьютеру.
//! Windows: перебираем диски, у которых есть system\.pocketbook; метка тома, ёмкость и то, как диск представляется по USB.
//! Для проверок на Linux: переменная INKMAIDEN_FAKE_DRIVES="/путь/к/папке=МЕТКА;…".

use pb_core::devices::{is_pocketbook, kernel_from_rev, scan_root, Seen};
use pb_core::models::Models;
use std::path::PathBuf;

/// Диск читалки: корень и метка тома
#[derive(Clone, Debug, PartialEq)]
pub struct Drive { pub root: PathBuf, pub label: String }

#[cfg(windows)]
mod win {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::*;
    use windows_sys::Win32::System::Ioctl::*;
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::WindowsProgramming::{DRIVE_FIXED, DRIVE_REMOVABLE};

    fn wide(s: &str) -> Vec<u16> { std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect() }

    pub fn drives() -> Vec<String> {
        let mask = unsafe { GetLogicalDrives() };
        (2..26u32).filter(|i| mask & (1 << i) != 0).map(|i| format!("{}:\\", (b'A' + i as u8) as char))
            .filter(|r| { let t = unsafe { GetDriveTypeW(wide(r).as_ptr()) }; t == DRIVE_REMOVABLE || t == DRIVE_FIXED }).collect()
    }
    pub fn label(root: &str) -> String {
        let mut name = [0u16; 261];
        let ok = unsafe { GetVolumeInformationW(wide(root).as_ptr(), name.as_mut_ptr(), 261, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0) };
        if ok == 0 { return String::new() }
        let n = name.iter().position(|&c| c == 0).unwrap_or(0);
        String::from_utf16_lossy(&name[..n])
    }
    pub fn space(root: &str) -> Option<(u64, u64)> {
        let (mut free, mut total, mut _f) = (0u64, 0u64, 0u64);
        let ok = unsafe { GetDiskFreeSpaceExW(wide(root).as_ptr(), &mut free, &mut total, &mut _f) };
        if ok == 0 { None } else { Some((total, free)) }
    }
    /// Как диск представляется: изготовитель, изделие, ревизия («USB-FS», «PocketBook», «0409»)
    pub fn descriptor(root: &str) -> Option<(String, String, String)> {
        let path = format!("\\\\.\\{}", &root[..2]);
        let h = unsafe { CreateFileW(wide(&path).as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
        if h == INVALID_HANDLE_VALUE { return None }
        let q = STORAGE_PROPERTY_QUERY { PropertyId: StorageDeviceProperty, QueryType: PropertyStandardQuery, AdditionalParameters: [0] };
        let mut buf = vec![0u8; 1024];
        let mut got = 0u32;
        let ok = unsafe { DeviceIoControl(h, IOCTL_STORAGE_QUERY_PROPERTY, &q as *const _ as *const _, std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut got, std::ptr::null_mut()) };
        unsafe { CloseHandle(h) };
        if ok == 0 || (got as usize) < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() { return None }
        let d = unsafe { &*(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };
        let s = |off: u32| -> String {
            if off == 0 || off as usize >= got as usize { return String::new() }
            let b = &buf[off as usize..got as usize];
            let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            String::from_utf8_lossy(&b[..n]).trim().to_string()
        };
        Some((s(d.VendorIdOffset), s(d.ProductIdOffset), s(d.ProductRevisionOffset)))
    }

    fn open(path: &str, access: u32) -> Option<windows_sys::Win32::Foundation::HANDLE> {
        let h = unsafe { CreateFileW(wide(path).as_ptr(), access, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
        (h != INVALID_HANDLE_VALUE).then_some(h)
    }
    fn ioctl<T>(h: windows_sys::Win32::Foundation::HANDLE, code: u32, out: &mut T) -> bool {
        let mut got = 0u32;
        unsafe { DeviceIoControl(h, code, std::ptr::null(), 0, out as *mut T as *mut _, std::mem::size_of::<T>() as u32, &mut got, std::ptr::null_mut()) != 0 }
    }

    /// Файловая система, кластер, «грязный» том, размер раздела и диска, поддержка SMART — только чтение
    pub fn disk(root: &str) -> serde_json::Value {
        use serde_json::json;
        let mut v = json!({});
        let (mut serial, mut maxc, mut flags) = (0u32, 0u32, 0u32);
        let mut fs = [0u16; 64];
        if unsafe { GetVolumeInformationW(wide(root).as_ptr(), std::ptr::null_mut(), 0, &mut serial, &mut maxc, &mut flags, fs.as_mut_ptr(), 64) } != 0 {
            let n = fs.iter().position(|&c| c == 0).unwrap_or(0);
            v["fs"] = json!(String::from_utf16_lossy(&fs[..n]));
            v["volumeSerial"] = json!(format!("{:04X}-{:04X}", serial >> 16, serial & 0xFFFF));
        }
        let (mut spc, mut bps, mut fc, mut tc) = (0u32, 0u32, 0u32, 0u32);
        if unsafe { GetDiskFreeSpaceW(wide(root).as_ptr(), &mut spc, &mut bps, &mut fc, &mut tc) } != 0 {
            v["cluster"] = json!(spc as u64 * bps as u64); v["sector"] = json!(bps); v["clusters"] = json!(tc); v["freeClusters"] = json!(fc);
        }
        let vol = format!("\\\\.\\{}", &root[..2]);
        if let Some(h) = open(&vol, 0) {
            let mut dirty = 0u32;
            if ioctl(h, FSCTL_IS_VOLUME_DIRTY, &mut dirty) { v["dirty"] = json!(dirty & VOLUME_IS_DIRTY != 0) }
            let mut len = GET_LENGTH_INFORMATION { Length: 0 };
            if ioctl(h, IOCTL_DISK_GET_LENGTH_INFO, &mut len) { v["partition"] = json!(len.Length) }
            let mut num: STORAGE_DEVICE_NUMBER = unsafe { std::mem::zeroed() };
            if ioctl(h, IOCTL_STORAGE_GET_DEVICE_NUMBER, &mut num) {
                v["diskNumber"] = json!(num.DeviceNumber);
                if let Some(ph) = open(&format!("\\\\.\\PhysicalDrive{}", num.DeviceNumber), 0) {
                    let mut g: DISK_GEOMETRY_EX = unsafe { std::mem::zeroed() };
                    if ioctl(ph, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, &mut g) { v["diskSize"] = json!(g.DiskSize) }
                    let mut sv: GETVERSIONINPARAMS = unsafe { std::mem::zeroed() };
                    v["smart"] = json!(ioctl(ph, SMART_GET_VERSION, &mut sv));
                    unsafe { CloseHandle(ph) };
                }
            }
            unsafe { CloseHandle(h) };
        }
        let path = format!("\\\\.\\{}", &root[..2]);
        if let Some(h) = open(&path, 0) {
            let q = STORAGE_PROPERTY_QUERY { PropertyId: StorageDeviceProperty, QueryType: PropertyStandardQuery, AdditionalParameters: [0] };
            let mut buf = vec![0u8; 1024];
            let mut got = 0u32;
            let ok = unsafe { DeviceIoControl(h, IOCTL_STORAGE_QUERY_PROPERTY, &q as *const _ as *const _, std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut got, std::ptr::null_mut()) };
            unsafe { CloseHandle(h) };
            if ok != 0 && got as usize >= std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
                let d = unsafe { &*(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };
                v["removable"] = json!(d.RemovableMedia);
                v["bus"] = json!(match d.BusType { 7 => "USB".to_string(), 11 => "SATA".into(), 12 => "SD".into(), 13 => "MMC".into(), 17 => "NVMe".into(), x => format!("код {x}") });
                let off = d.SerialNumberOffset as usize;
                if off > 0 && off < got as usize { let b = &buf[off..got as usize]; let n = b.iter().position(|&c| c == 0).unwrap_or(b.len()); v["serial"] = json!(String::from_utf8_lossy(&b[..n]).trim()) }
            }
        }
        v
    }

    /// Безопасно извлечь: сбросить буферы, заблокировать и отключить том, вытолкнуть носитель
    pub fn eject(root: &str) -> Result<(), String> {
        use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
        let vol = format!("\\\\.\\{}", &root[..2]);
        let h = open(&vol, GENERIC_READ | GENERIC_WRITE).ok_or("Не удалось открыть диск читалки.")?;
        let call = |code: u32, input: *const core::ffi::c_void, len: u32| -> bool {
            let mut got = 0u32;
            unsafe { DeviceIoControl(h, code, input, len, std::ptr::null_mut(), 0, &mut got, std::ptr::null_mut()) != 0 }
        };
        unsafe { FlushFileBuffers(h) };
        let mut locked = false;
        for _ in 0..20 { if call(FSCTL_LOCK_VOLUME, std::ptr::null(), 0) { locked = true; break } std::thread::sleep(std::time::Duration::from_millis(250)) }
        if !locked { unsafe { CloseHandle(h) }; return Err("Диск занят: закройте окна Проводника и программы, которые открыли файлы на читалке, и попробуйте снова.".into()) }
        let ok = call(FSCTL_DISMOUNT_VOLUME, std::ptr::null(), 0);
        let pmr = PREVENT_MEDIA_REMOVAL { PreventMediaRemoval: false };
        call(IOCTL_STORAGE_MEDIA_REMOVAL, &pmr as *const _ as *const _, std::mem::size_of::<PREVENT_MEDIA_REMOVAL>() as u32);
        let ej = call(IOCTL_STORAGE_EJECT_MEDIA, std::ptr::null(), 0);
        unsafe { CloseHandle(h) };
        if ok && ej { Ok(()) } else { Err("Windows не дала извлечь читалку — извлеките её через значок в области уведомлений.".into()) }
    }
}

/// Подключённые читалки (только перечень — быстро, раз в пару секунд)
/// Буквы дисков, не трогая сами диски (для «извлечённых»: к ним нельзя обращаться, иначе Windows подключит том снова)
pub fn letters() -> Vec<String> {
    #[cfg(windows)] { win::drives() }
    #[cfg(not(windows))] { std::env::var("INKMAIDEN_FAKE_DRIVES").unwrap_or_default().split(';').filter_map(|s| s.split_once('=')).map(|(p, _)| p.to_string()).collect() }
}

/// Подключённые читалки, кроме безопасно извлечённых (skip — их корни)
pub fn list_except(skip: &std::collections::HashSet<String>) -> Vec<Drive> {
    #[cfg(windows)]
    {
        win::drives().into_iter().filter(|r| !skip.contains(r)).filter(|r| is_pocketbook(std::path::Path::new(r)))
            .map(|r| Drive { label: win::label(&r), root: PathBuf::from(r) }).collect()
    }
    #[cfg(not(windows))]
    {
        std::env::var("INKMAIDEN_FAKE_DRIVES").unwrap_or_default().split(';').filter(|s| !s.is_empty())
            .filter_map(|s| s.split_once('=')).filter(|(p, _)| !skip.contains(*p)).map(|(p, l)| Drive { root: PathBuf::from(p), label: l.into() })
            .filter(|d| is_pocketbook(&d.root)).collect()
    }
}

/// Всё, что видно о читалке на этом диске
pub fn inspect(d: &Drive, models: &Models) -> Seen {
    let mut s = scan_root(&d.root, &d.label, models);
    #[cfg(windows)]
    {
        let r = d.root.display().to_string();
        if let Some((t, f)) = win::space(&r) { s.total = Some(t); s.free = Some(f) }
        if let Some((v, p, rev)) = win::descriptor(&r) {
            s.kernel = kernel_from_rev(&rev);
            s.usb_vendor = Some(v).filter(|x| !x.is_empty());
            s.usb_product = Some(p).filter(|x| !x.is_empty());
            s.usb_revision = Some(rev).filter(|x| !x.is_empty());
        }
    }
    s
}

/// Свободно и всего на диске (Windows)
pub fn space(root: &std::path::Path) -> Option<(u64, u64)> {
    #[cfg(windows)] { win::space(&root.display().to_string()).map(|(t, f)| (f, t)) }
    #[cfg(not(windows))] { let _ = root; None }
}

/// Сведения о диске читалки со стороны Windows (файловая система, кластер, проверка тома, размеры, SMART)
pub fn disk(root: &std::path::Path) -> serde_json::Value {
    #[cfg(windows)] { win::disk(&root.display().to_string()) }
    #[cfg(not(windows))] { let _ = root; serde_json::Value::Null }
}

/// Безопасное извлечение (Windows)
pub fn eject(root: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)] { win::eject(&root.display().to_string()) }
    #[cfg(not(windows))] { let _ = root; Err("Безопасное извлечение есть только в Windows.".into()) }
}
