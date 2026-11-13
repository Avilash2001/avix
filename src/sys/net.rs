//! Latency probe (ICMP echo) and active network adapter inspection.

use super::{from_wide_ptr, OwnedHandle};
use std::ffi::c_void;
use std::net::{Ipv4Addr, ToSocketAddrs};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, GAA_FLAG_INCLUDE_GATEWAYS,
    ICMP_ECHO_REPLY, IP_ADAPTER_ADDRESSES_LH,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;

pub fn resolve_ipv4(host: &str) -> Option<Ipv4Addr> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Some(ip);
    }
    (host, 0u16).to_socket_addrs().ok()?.find_map(|a| match a.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    })
}

pub struct Pinger {
    handle: *mut c_void,
    ip: Ipv4Addr,
}

// The ICMP handle is only used from the thread that owns the Pinger.
unsafe impl Send for Pinger {}

impl Pinger {
    pub fn new(host: &str) -> Option<Self> {
        let ip = resolve_ipv4(host)?;
        let h = unsafe { IcmpCreateFile() };
        // validate (IcmpCreateFile returns INVALID_HANDLE_VALUE on failure)
        let owned = OwnedHandle::new(h)?;
        std::mem::forget(owned); // closed with IcmpCloseHandle in Drop
        Some(Self { handle: h, ip })
    }

    /// Round-trip time in ms, or None on timeout/loss.
    pub fn ping(&self, timeout_ms: u32) -> Option<u32> {
        let payload = [0x41u8; 32];
        let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + payload.len() + 64];
        let n = unsafe {
            IcmpSendEcho(
                self.handle,
                u32::from_ne_bytes(self.ip.octets()),
                payload.as_ptr() as *const c_void,
                payload.len() as u16,
                std::ptr::null(),
                reply.as_mut_ptr() as *mut c_void,
                reply.len() as u32,
                timeout_ms,
            )
        };
        if n == 0 {
            return None;
        }
        let r = unsafe { &*(reply.as_ptr() as *const ICMP_ECHO_REPLY) };
        (r.Status == 0).then_some(r.RoundTripTime)
    }
}

impl Drop for Pinger {
    fn drop(&mut self) {
        unsafe {
            IcmpCloseHandle(self.handle);
        }
    }
}

#[derive(Clone, Debug)]
pub struct AdapterInfo {
    pub name: String,
    pub wifi: bool,
    pub speed_mbps: u64,
}

/// The adapter(s) that are up and have a default gateway, i.e. carry internet traffic.
pub fn active_adapters() -> Vec<AdapterInfo> {
    let mut out = Vec::new();
    let mut size = 32 * 1024u32;
    let mut buf: Vec<u64> = Vec::new();
    for _ in 0..3 {
        buf = vec![0u64; size as usize / 8 + 1];
        let rc = unsafe {
            GetAdaptersAddresses(
                0, // AF_UNSPEC
                GAA_FLAG_INCLUDE_GATEWAYS,
                std::ptr::null(),
                buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                &mut size,
            )
        };
        if rc == 0 {
            break;
        }
        if rc != 111 {
            // not ERROR_BUFFER_OVERFLOW
            return out;
        }
    }
    let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !p.is_null() {
        let a = unsafe { &*p };
        if a.OperStatus == IfOperStatusUp && !a.FirstGatewayAddress.is_null() && a.IfType != 24 {
            out.push(AdapterInfo {
                name: unsafe { from_wide_ptr(a.FriendlyName) },
                wifi: a.IfType == 71,
                speed_mbps: a.ReceiveLinkSpeed / 1_000_000,
            });
        }
        p = a.Next;
    }
    out
}
