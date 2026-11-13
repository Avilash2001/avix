//! Power plans via powrprof (no powercfg.exe spawning).

use super::error_text;
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
use windows_sys::Win32::System::Power::{
    PowerDuplicateScheme, PowerEnumerate, PowerGetActiveScheme, PowerReadFriendlyName,
    PowerSetActiveScheme, PowerWriteACValueIndex, PowerWriteDCValueIndex,
    PowerWriteFriendlyName, ACCESS_SCHEME,
};

pub const AVIX_PLAN_NAME: &str = "Avix Ultimate Performance";

pub const fn guid(u: u128) -> GUID {
    GUID {
        data1: (u >> 96) as u32,
        data2: (u >> 80) as u16,
        data3: (u >> 64) as u16,
        data4: (u as u64).to_be_bytes(),
    }
}

pub fn to_u128(g: &GUID) -> u128 {
    (g.data1 as u128) << 96
        | (g.data2 as u128) << 80
        | (g.data3 as u128) << 64
        | u64::from_be_bytes(g.data4) as u128
}

pub fn same(a: &GUID, b: &GUID) -> bool {
    to_u128(a) == to_u128(b)
}

pub fn to_string(g: &GUID) -> String {
    let u = to_u128(g);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (u >> 96) as u32,
        (u >> 80) as u16,
        (u >> 64) as u16,
        (u >> 48) as u16,
        u & 0xffff_ffff_ffff
    )
}

pub fn parse(s: &str) -> Option<GUID> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    (hex.len() == 32).then(|| u128::from_str_radix(&hex, 16).ok().map(guid)).flatten()
}

pub const ULTIMATE: GUID = guid(0xe9a42b02_d5df_448d_aa00_03f14749eb61);
pub const HIGH_PERFORMANCE: GUID = guid(0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c);
pub const BALANCED: GUID = guid(0x381b4222_f694_41f0_9685_ff5bb260df2e);

// (subgroup, setting, value) tuned on the Avix plan.
const TUNING: &[(u128, u128, u32)] = &[
    // USB selective suspend: disabled (avoids mice/keyboards/headsets napping)
    (0x2a737441_1930_4402_8d77_b2bebba308a3, 0x48e6b7a6_50f5_4782_a5d4_53bb8f07e226, 0),
    // PCI Express Link State Power Management: off
    (0x501a4d13_42af_4429_9fd1_a8218c268e20, 0xee12f906_d277_404b_b6da_e5fa1a576df5, 0),
    // Minimum processor state: 100 %
    (0x54533251_82be_4824_96c1_47b60b740d00, 0x893dee8e_2bef_41e0_89c6_b55d0929964c, 100),
    // Core parking: keep 100 % of cores unparked
    (0x54533251_82be_4824_96c1_47b60b740d00, 0x0cc5b647_c1df_4637_891a_dec35c318583, 100),
    // Turn off hard disk after: never
    (0x0012ee47_9041_4b5d_9b77_535fba8b1442, 0x6738e2c4_e8a5_4a42_b16a_e040e769756e, 0),
    // Sleep after: never (long renders / model runs)
    (0x238c9fa8_0aad_41ed_83f4_97be242c8f20, 0x29f6c1db_86da_48c5_9fdb_f2b67b1f44da, 0),
];

pub fn active() -> Option<GUID> {
    unsafe {
        let mut p: *mut GUID = std::ptr::null_mut();
        if PowerGetActiveScheme(std::ptr::null_mut(), &mut p) != ERROR_SUCCESS || p.is_null() {
            return None;
        }
        let g = *p;
        LocalFree(p as _);
        Some(g)
    }
}

pub fn set_active(g: &GUID) -> Result<(), String> {
    let rc = unsafe { PowerSetActiveScheme(std::ptr::null_mut(), g) };
    if rc == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(error_text(rc))
    }
}

pub fn name_of(g: &GUID) -> String {
    unsafe {
        let mut size = 0u32;
        PowerReadFriendlyName(
            std::ptr::null_mut(),
            g,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut size,
        );
        if size == 0 {
            return to_string(g);
        }
        let mut buf = vec![0u8; size as usize];
        if PowerReadFriendlyName(
            std::ptr::null_mut(),
            g,
            std::ptr::null(),
            std::ptr::null(),
            buf.as_mut_ptr(),
            &mut size,
        ) != ERROR_SUCCESS
        {
            return to_string(g);
        }
        let u: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        super::from_wide(&u)
    }
}

pub fn list() -> Vec<(GUID, String)> {
    let mut out = Vec::new();
    let mut i = 0u32;
    loop {
        let mut g: GUID = guid(0);
        let mut size = std::mem::size_of::<GUID>() as u32;
        let rc = unsafe {
            PowerEnumerate(
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                ACCESS_SCHEME,
                i,
                &mut g as *mut GUID as *mut u8,
                &mut size,
            )
        };
        if rc != ERROR_SUCCESS {
            break;
        }
        out.push((g, name_of(&g)));
        i += 1;
    }
    out
}

/// Find the Avix plan, or create it by duplicating Ultimate Performance
/// (falling back to High Performance) and tuning latency-relevant settings.
pub fn ensure_avix_plan() -> Result<GUID, String> {
    if let Some((g, _)) = list().into_iter().find(|(_, n)| n == AVIX_PLAN_NAME) {
        return Ok(g);
    }
    let mut created: Option<GUID> = None;
    for src in [ULTIMATE, HIGH_PERFORMANCE] {
        unsafe {
            let mut p: *mut GUID = std::ptr::null_mut();
            if PowerDuplicateScheme(std::ptr::null_mut(), &src, &mut p) == ERROR_SUCCESS && !p.is_null() {
                created = Some(*p);
                LocalFree(p as _);
                break;
            }
        }
    }
    let g = created.ok_or_else(|| "this PC does not allow custom power plans".to_string())?;
    let name: Vec<u8> = super::wide(AVIX_PLAN_NAME).iter().flat_map(|c| c.to_le_bytes()).collect();
    unsafe {
        PowerWriteFriendlyName(
            std::ptr::null_mut(),
            &g,
            std::ptr::null(),
            std::ptr::null(),
            name.as_ptr(),
            name.len() as u32,
        );
        for &(sub, setting, value) in TUNING {
            let (sub, setting) = (guid(sub), guid(setting));
            PowerWriteACValueIndex(std::ptr::null_mut(), &g, &sub, &setting, value);
            PowerWriteDCValueIndex(std::ptr::null_mut(), &g, &sub, &setting, value);
        }
    }
    Ok(g)
}

/// Delete the Avix plan (switching to Balanced first if it's active).
pub fn remove_avix_plan() -> Result<bool, String> {
    use windows_sys::Win32::System::Power::PowerDeleteScheme;
    let Some((g, _)) = list().into_iter().find(|(_, n)| n == AVIX_PLAN_NAME) else {
        return Ok(false);
    };
    if active().is_some_and(|a| same(&a, &g)) {
        set_active(&BALANCED)?;
    }
    let rc = unsafe { PowerDeleteScheme(std::ptr::null_mut(), &g) };
    if rc == ERROR_SUCCESS {
        Ok(true)
    } else {
        Err(error_text(rc))
    }
}

/// Read a plan's AC value for (subgroup, setting).
#[cfg(debug_assertions)]
pub fn read_ac(g: &GUID, sub: u128, setting: u128) -> Option<u32> {
    use windows_sys::Win32::System::Power::PowerReadACValueIndex;
    let (sub, setting) = (guid(sub), guid(setting));
    let mut v = 0u32;
    let rc = unsafe { PowerReadACValueIndex(std::ptr::null_mut(), g, &sub, &setting, &mut v) };
    (rc == ERROR_SUCCESS).then_some(v)
}

pub fn is_performance_plan(g: &GUID) -> bool {
    same(g, &ULTIMATE) || same(g, &HIGH_PERFORMANCE) || name_of(g) == AVIX_PLAN_NAME
}
