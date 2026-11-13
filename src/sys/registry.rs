//! Registry helpers (always the 64-bit view).

use super::{error_text, wide};
use serde::{Deserialize, Serialize};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW,
    RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_ENUMERATE_SUB_KEYS, KEY_QUERY_VALUE, KEY_SET_VALUE, KEY_WOW64_64KEY, REG_BINARY,
    REG_DWORD, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_QWORD, REG_SZ,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hive {
    HKLM,
    HKCU,
}

impl Hive {
    fn raw(self) -> HKEY {
        match self {
            Hive::HKLM => HKEY_LOCAL_MACHINE,
            Hive::HKCU => HKEY_CURRENT_USER,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RegValue {
    Dword(u32),
    Qword(u64),
    Str(String),
    ExpandStr(String),
    Binary(Vec<u8>),
}

impl RegValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            RegValue::Str(s) | RegValue::ExpandStr(s) => Some(s),
            _ => None,
        }
    }
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn open(hive: Hive, path: &str, access: u32) -> Option<Key> {
    let mut hk: HKEY = std::ptr::null_mut();
    let p = wide(path);
    let rc = unsafe { RegOpenKeyExW(hive.raw(), p.as_ptr(), 0, access | KEY_WOW64_64KEY, &mut hk) };
    (rc == ERROR_SUCCESS).then_some(Key(hk))
}

fn create(hive: Hive, path: &str) -> Result<Key, String> {
    let mut hk: HKEY = std::ptr::null_mut();
    let p = wide(path);
    let rc = unsafe {
        RegCreateKeyExW(
            hive.raw(),
            p.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE | KEY_QUERY_VALUE | KEY_WOW64_64KEY,
            std::ptr::null(),
            &mut hk,
            std::ptr::null_mut(),
        )
    };
    if rc == ERROR_SUCCESS {
        Ok(Key(hk))
    } else {
        Err(error_text(rc))
    }
}

fn decode(ty: u32, buf: &[u8]) -> RegValue {
    match ty {
        REG_DWORD if buf.len() >= 4 => RegValue::Dword(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])),
        REG_QWORD if buf.len() >= 8 => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&buf[..8]);
            RegValue::Qword(u64::from_le_bytes(b))
        }
        REG_SZ | REG_EXPAND_SZ => {
            let u: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let s = super::from_wide(&u);
            if ty == REG_SZ { RegValue::Str(s) } else { RegValue::ExpandStr(s) }
        }
        _ => RegValue::Binary(buf.to_vec()),
    }
}

fn encode(v: &RegValue) -> (u32, Vec<u8>) {
    match v {
        RegValue::Dword(d) => (REG_DWORD, d.to_le_bytes().to_vec()),
        RegValue::Qword(q) => (REG_QWORD, q.to_le_bytes().to_vec()),
        RegValue::Str(s) => (REG_SZ, wide(s).iter().flat_map(|c| c.to_le_bytes()).collect()),
        RegValue::ExpandStr(s) => (REG_EXPAND_SZ, wide(s).iter().flat_map(|c| c.to_le_bytes()).collect()),
        RegValue::Binary(b) => (REG_BINARY, b.clone()),
    }
}

fn query(key: &Key, name: &str) -> Option<RegValue> {
    let n = wide(name);
    let mut ty = 0u32;
    let mut size = 0u32;
    unsafe {
        if RegQueryValueExW(key.0, n.as_ptr(), std::ptr::null(), &mut ty, std::ptr::null_mut(), &mut size)
            != ERROR_SUCCESS
        {
            return None;
        }
        let mut buf = vec![0u8; size as usize + 2];
        let mut size2 = buf.len() as u32;
        if RegQueryValueExW(key.0, n.as_ptr(), std::ptr::null(), &mut ty, buf.as_mut_ptr(), &mut size2)
            != ERROR_SUCCESS
        {
            return None;
        }
        buf.truncate(size2 as usize);
        Some(decode(ty, &buf))
    }
}

pub fn get(hive: Hive, path: &str, name: &str) -> Option<RegValue> {
    let key = open(hive, path, KEY_QUERY_VALUE)?;
    query(&key, name)
}

pub fn get_dword(hive: Hive, path: &str, name: &str) -> Option<u32> {
    match get(hive, path, name)? {
        RegValue::Dword(d) => Some(d),
        _ => None,
    }
}

pub fn get_string(hive: Hive, path: &str, name: &str) -> Option<String> {
    get(hive, path, name).and_then(|v| v.as_str().map(str::to_string))
}

pub fn set(hive: Hive, path: &str, name: &str, value: &RegValue) -> Result<(), String> {
    let key = create(hive, path)?;
    let n = wide(name);
    let (ty, data) = encode(value);
    let rc = unsafe { RegSetValueExW(key.0, n.as_ptr(), 0, ty, data.as_ptr(), data.len() as u32) };
    if rc == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(error_text(rc))
    }
}

pub fn set_dword(hive: Hive, path: &str, name: &str, v: u32) -> Result<(), String> {
    set(hive, path, name, &RegValue::Dword(v))
}

/// Deleting a value that does not exist is a success.
pub fn delete(hive: Hive, path: &str, name: &str) -> Result<(), String> {
    let Some(key) = open(hive, path, KEY_SET_VALUE) else {
        return Ok(());
    };
    let n = wide(name);
    let rc = unsafe { RegDeleteValueW(key.0, n.as_ptr()) };
    if rc == ERROR_SUCCESS || rc == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(error_text(rc))
    }
}

pub fn key_exists(hive: Hive, path: &str) -> bool {
    open(hive, path, KEY_QUERY_VALUE).is_some()
}

/// Ancestors of `path` (including itself) that don't exist yet, deepest first.
/// Lets a revert remove keys that only exist because Avix created them.
pub fn missing_keys(hive: Hive, path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('\\').filter(|p| !p.is_empty()).collect();
    let mut missing = Vec::new();
    for i in (1..=parts.len()).rev() {
        let prefix = parts[..i].join("\\");
        if key_exists(hive, &prefix) {
            break;
        }
        missing.push(prefix);
    }
    missing
}

/// Delete a key only if it has no values and no subkeys. Returns true if deleted.
pub fn delete_key_if_empty(hive: Hive, path: &str) -> bool {
    use windows_sys::Win32::System::Registry::{RegDeleteKeyExW, RegQueryInfoKeyW};
    {
        let Some(key) = open(hive, path, KEY_QUERY_VALUE | KEY_ENUMERATE_SUB_KEYS) else { return false };
        let (mut subkeys, mut vals) = (0u32, 0u32);
        let rc = unsafe {
            RegQueryInfoKeyW(
                key.0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                &mut subkeys,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut vals,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc != ERROR_SUCCESS || subkeys != 0 || vals != 0 {
            return false;
        }
    }
    let p = wide(path);
    unsafe { RegDeleteKeyExW(hive.raw(), p.as_ptr(), KEY_WOW64_64KEY, 0) == ERROR_SUCCESS }
}

pub fn subkeys(hive: Hive, path: &str) -> Vec<String> {
    let Some(key) = open(hive, path, KEY_ENUMERATE_SUB_KEYS) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut i = 0u32;
    loop {
        let mut name = [0u16; 512];
        let mut len = name.len() as u32;
        let rc = unsafe {
            RegEnumKeyExW(
                key.0,
                i,
                name.as_mut_ptr(),
                &mut len,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc != ERROR_SUCCESS {
            break;
        }
        out.push(String::from_utf16_lossy(&name[..len as usize]));
        i += 1;
    }
    out
}

pub fn values(hive: Hive, path: &str) -> Vec<(String, RegValue)> {
    let Some(key) = open(hive, path, KEY_QUERY_VALUE) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut i = 0u32;
    let mut data = vec![0u8; 4096];
    loop {
        let mut name = vec![0u16; 16384];
        let mut name_len = name.len() as u32;
        let mut ty = 0u32;
        let mut data_len = data.len() as u32;
        let rc = unsafe {
            RegEnumValueW(
                key.0,
                i,
                name.as_mut_ptr(),
                &mut name_len,
                std::ptr::null(),
                &mut ty,
                data.as_mut_ptr(),
                &mut data_len,
            )
        };
        if rc == windows_sys::Win32::Foundation::ERROR_MORE_DATA {
            data.resize(data_len as usize + 16, 0);
            continue;
        }
        if rc != ERROR_SUCCESS {
            break;
        }
        let n = String::from_utf16_lossy(&name[..name_len as usize]);
        out.push((n, decode(ty, &data[..data_len as usize])));
        i += 1;
    }
    out
}
