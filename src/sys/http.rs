//! Minimal HTTPS GET over WinHTTP (built into Windows: no TLS library, no extra crates).
//! Used only for the once-a-day update check.

use super::wide;
use std::ffi::c_void;
use windows_sys::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
    WINHTTP_QUERY_STATUS_CODE,
};

struct H(*mut c_void);

impl Drop for H {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn ok(h: *mut c_void, what: &str) -> Result<H, String> {
    if h.is_null() {
        Err(format!("{what} failed ({})", super::error_text(super::last_error())))
    } else {
        Ok(H(h))
    }
}

/// GET https://{host}{path}; returns the body of a 200 response (capped at `max` bytes).
pub fn get(host: &str, path: &str, headers: &str, max: usize) -> Result<String, String> {
    let agent = wide(&format!("Avix/{}", crate::VERSION));
    let host_w = wide(host);
    let verb = wide("GET");
    let path_w = wide(path);
    let hdr: Vec<u16> = headers.encode_utf16().collect();
    unsafe {
        let session = ok(
            WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, std::ptr::null(), std::ptr::null(), 0),
            "WinHttpOpen",
        )?;
        WinHttpSetTimeouts(session.0, 10_000, 10_000, 10_000, 15_000);
        let conn = ok(WinHttpConnect(session.0, host_w.as_ptr(), 443, 0), "connect")?;
        let req = ok(
            WinHttpOpenRequest(
                conn.0,
                verb.as_ptr(),
                path_w.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            ),
            "request",
        )?;
        let (hp, hl) = if hdr.is_empty() { (std::ptr::null(), 0) } else { (hdr.as_ptr(), hdr.len() as u32) };
        if WinHttpSendRequest(req.0, hp, hl, std::ptr::null(), 0, 0, 0) == 0 {
            return Err(format!("couldn't reach {host} ({})", super::error_text(super::last_error())));
        }
        if WinHttpReceiveResponse(req.0, std::ptr::null_mut()) == 0 {
            return Err(format!("no response from {host} ({})", super::error_text(super::last_error())));
        }
        let mut code = 0u32;
        let mut len = 4u32;
        WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut code as *mut u32 as *mut c_void,
            &mut len,
            std::ptr::null_mut(),
        );
        if code != 200 {
            return Err(format!("{host} answered HTTP {code}"));
        }
        let mut body = Vec::new();
        loop {
            let mut avail = 0u32;
            if WinHttpQueryDataAvailable(req.0, &mut avail) == 0 || avail == 0 {
                break;
            }
            let start = body.len();
            body.resize(start + avail as usize, 0);
            let mut read = 0u32;
            if WinHttpReadData(req.0, body[start..].as_mut_ptr() as *mut c_void, avail, &mut read) == 0 {
                return Err("download interrupted".into());
            }
            body.truncate(start + read as usize);
            if body.len() > max {
                return Err("response too large".into());
            }
        }
        String::from_utf8(body).map_err(|_| "response wasn't text".to_string())
    }
}
