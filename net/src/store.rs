#[cfg(not(target_os = "emscripten"))]
mod imp {
    fn path(key: &str) -> Option<std::path::PathBuf> {
        let mut p = dirs::config_dir()?;
        p.push("wrum");
        std::fs::create_dir_all(&p).ok()?;
        p.push(key);
        Some(p)
    }
    pub fn load(key: &str) -> Option<String> {
        let s = std::fs::read_to_string(path(key)?).ok()?;
        let s = s.trim().to_string();
        (!s.is_empty()).then_some(s)
    }
    pub fn save(key: &str, val: &str) {
        if let Some(p) = path(key) {
            let _ = std::fs::write(p, val);
        }
    }
}

#[cfg(target_os = "emscripten")]
mod imp {
    use std::{
        ffi::CString,
        os::raw::{c_char, c_int},
    };
    unsafe extern "C" {
        fn store_get(key: *const c_char, buf: *mut u8, cap: c_int) -> c_int;
        fn store_set(key: *const c_char, val: *const c_char);
    }
    pub fn load(key: &str) -> Option<String> {
        let k = CString::new(key).ok()?;
        let mut buf = vec![0u8; 256];
        let n = unsafe { store_get(k.as_ptr(), buf.as_mut_ptr(), 256) };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
    }
    pub fn save(key: &str, val: &str) {
        if let (Ok(k), Ok(v)) = (CString::new(key), CString::new(val)) {
            unsafe { store_set(k.as_ptr(), v.as_ptr()) };
        }
    }
}

pub use imp::{load, save};
