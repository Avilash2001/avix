//! Hardware facts for the PC check, read without WMI:
//! - RAM sticks from the firmware's SMBIOS table (size, rated vs configured speed, slot),
//! - graphics adapters from Windows' device list (name, driver date, integrated or dedicated),
//! - whether a drive is a hard disk, and whether a laptop is on battery.

use super::registry::{self, Hive};
use super::wide;

// ------------------------------------------------------------------ RAM (SMBIOS type 17)

#[derive(Clone, Debug, Default)]
pub struct MemStick {
    pub size_mb: u64,
    /// SMBIOS form factor: 0x09 DIMM (desktop), 0x0D SODIMM (laptop), 0x0F row of chips (soldered)
    pub form_factor: u8,
    /// 0x18 DDR3, 0x1A DDR4, 0x1E LPDDR4, 0x22 DDR5, 0x23 LPDDR5
    pub mem_type: u8,
    /// Best speed the stick is rated for (MT/s), from SMBIOS or its part number (XMP/EXPO kits).
    pub rated: u32,
    /// Speed it actually runs at (MT/s).
    pub configured: u32,
    pub locator: String,
    pub bank: String,
    pub part: String,
}

impl MemStick {
    pub fn is_desktop_dimm(&self) -> bool {
        self.form_factor == 0x09
    }

    pub fn ddr_name(&self) -> &'static str {
        match self.mem_type {
            0x18 => "DDR3",
            0x1A => "DDR4",
            0x1D => "LPDDR3",
            0x1E => "LPDDR4",
            0x22 => "DDR5",
            0x23 => "LPDDR5",
            _ => "RAM",
        }
    }

    /// Memory channel letter if the slot names make it clear ("ChannelA-DIMM0", "DIMM_B2", ...).
    pub fn channel(&self) -> Option<char> {
        for s in [&self.bank, &self.locator] {
            let u = s.to_uppercase();
            if let Some(i) = u.find("CHANNEL") {
                let c = u[i + 7..].trim_start_matches([' ', '_', '-']).chars().next()?;
                if c.is_ascii_uppercase() {
                    return Some(c);
                }
            }
        }
        // "DIMM_A1", "DIMM A2", "DIMMB1"
        let u = self.locator.to_uppercase();
        let rest = u.strip_prefix("DIMM")?.trim_start_matches([' ', '_', '-']);
        let mut it = rest.chars();
        match (it.next(), it.next()) {
            (Some(c), Some(d)) if c.is_ascii_uppercase() && d.is_ascii_digit() => Some(c),
            _ => None,
        }
    }
}

/// Common DDR3-DDR5 speeds (MT/s); used to spot the rated speed inside part numbers.
const SPEEDS: &[u32] = &[
    1333, 1600, 1866, 2133, 2400, 2666, 2800, 2933, 3000, 3200, 3333, 3466, 3600, 3733, 3866, 4000, 4133, 4266,
    4400, 4600, 4800, 5200, 5600, 6000, 6200, 6400, 6600, 6800, 7000, 7200, 7600, 8000, 8200, 8400,
];

/// "CMK16GX4M2B3200C16" -> 3200, "F4-3600C16D-16GVKC" -> 3600. Kits that hide the speed return 0.
pub fn part_number_speed(part: &str) -> u32 {
    let b = part.as_bytes();
    let mut best = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i - start == 4 {
                if let Ok(v) = part[start..i].parse::<u32>() {
                    if SPEEDS.contains(&v) {
                        best = best.max(v);
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    best
}

/// Old firmware reports the clock in MHz (half the MT/s figure); normalise to MT/s.
fn to_mts(v: u32, mem_type: u8) -> u32 {
    let floor = match mem_type {
        0x22 | 0x23 => 3200,
        0x1A | 0x1E => 1600,
        _ => 800,
    };
    if v > 0 && v < floor {
        v * 2
    } else {
        v
    }
}

fn smbios_strings(area: &[u8]) -> Vec<String> {
    area.split(|&c| c == 0).map(|s| String::from_utf8_lossy(s).trim().to_string()).collect()
}

pub fn parse_smbios(raw: &[u8]) -> Vec<MemStick> {
    if raw.len() < 8 {
        return Vec::new();
    }
    let len = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]) as usize;
    let t = &raw[8..(8 + len).min(raw.len())];
    let u16_at = |s: &[u8], o: usize| if o + 2 <= s.len() { u16::from_le_bytes([s[o], s[o + 1]]) as u32 } else { 0 };
    let u32_at = |s: &[u8], o: usize| if o + 4 <= s.len() { u32::from_le_bytes([s[o], s[o + 1], s[o + 2], s[o + 3]]) } else { 0 };
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= t.len() {
        let kind = t[i];
        let flen = t[i + 1] as usize;
        if flen < 4 || i + flen > t.len() {
            break;
        }
        let f = &t[i..i + flen];
        let mut j = i + flen;
        while j + 1 < t.len() && !(t[j] == 0 && t[j + 1] == 0) {
            j += 1;
        }
        let strings = smbios_strings(&t[i + flen..j.min(t.len())]);
        let s = |idx: usize| if idx == 0 { String::new() } else { strings.get(idx - 1).cloned().unwrap_or_default() };
        if kind == 17 && flen >= 0x15 {
            let w = u16_at(f, 0x0C);
            let size_mb = match w {
                0 => 0,
                0xFFFF => 1,
                0x7FFF => u64::from(u32_at(f, 0x1C)),
                w if w & 0x8000 != 0 => u64::from(w & 0x7FFF) / 1024,
                w => u64::from(w),
            };
            if size_mb > 0 {
                let mem_type = f[0x12];
                let mut speed = u16_at(f, 0x15);
                if speed == 0xFFFF {
                    speed = u32_at(f, 0x54);
                }
                let mut configured = u16_at(f, 0x20);
                if configured == 0xFFFF {
                    configured = u32_at(f, 0x58);
                }
                let part = if flen > 0x1A { s(f[0x1A] as usize) } else { String::new() };
                let speed = to_mts(speed, mem_type);
                out.push(MemStick {
                    size_mb,
                    form_factor: f[0x0E],
                    mem_type,
                    rated: speed.max(part_number_speed(&part)),
                    configured: to_mts(configured, mem_type),
                    locator: s(f[0x10] as usize),
                    bank: s(f[0x11] as usize),
                    part,
                });
            }
        }
        if kind == 127 {
            break;
        }
        i = j + 2;
    }
    out
}

pub fn memory_sticks() -> Vec<MemStick> {
    use windows_sys::Win32::System::SystemInformation::GetSystemFirmwareTable;
    let rsmb = u32::from_be_bytes(*b"RSMB");
    unsafe {
        let n = GetSystemFirmwareTable(rsmb, 0, std::ptr::null_mut(), 0);
        if n == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; n as usize];
        if GetSystemFirmwareTable(rsmb, 0, buf.as_mut_ptr() as *mut _, n) != n {
            return Vec::new();
        }
        parse_smbios(&buf)
    }
}

// ------------------------------------------------------------------ graphics adapters

#[derive(Clone, Debug)]
pub struct Gpu {
    pub name: String,
    /// (year, month, day)
    pub driver_date: Option<(u32, u32, u32)>,
    pub driver_version: String,
    pub integrated: bool,
    /// NVIDIA, AMD or Intel (not a virtual / remote / basic display adapter)
    pub real: bool,
    pub basic_driver: bool,
}

fn classify(name: &str) -> (bool, bool, bool) {
    let n = name.to_lowercase();
    let basic = n.contains("microsoft basic display");
    let nvidia = n.contains("nvidia");
    let amd = n.contains("amd") || n.contains("radeon") || n.contains("ati ");
    let intel = n.contains("intel");
    let real = nvidia || amd || intel;
    let integrated = if intel {
        // Arc A/B-series cards are dedicated; "Arc Graphics" in Core Ultra laptops is integrated
        !["arc(tm) a", "arc(tm) b", "arc a", "arc b"].iter().any(|p| {
            n.find(p).is_some_and(|i| n[i + p.len()..].chars().next().is_some_and(|c| c.is_ascii_digit()))
        })
    } else if amd {
        !(n.contains(" rx") || n.contains("radeon pro") || n.contains("firepro") || n.contains("radeon r9") || n.contains("radeon r7"))
    } else {
        false
    };
    (real, integrated, basic)
}

fn parse_driver_date(s: &str) -> Option<(u32, u32, u32)> {
    // "7-12-2024" (month-day-year)
    let mut it = s.trim().split(['-', '/']).map(|x| x.parse::<u32>().ok());
    let (m, d, y) = (it.next()??, it.next()??, it.next()??);
    (y > 1990 && (1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// Display adapters that are present right now.
pub fn gpus() -> Vec<Gpu> {
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceRegistryPropertyW,
        DIGCF_PRESENT, GUID_DEVCLASS_DISPLAY, SPDRP_DEVICEDESC, SPDRP_DRIVER, SP_DEVINFO_DATA,
    };
    let mut out = Vec::new();
    unsafe {
        let set = SetupDiGetClassDevsW(&GUID_DEVCLASS_DISPLAY, std::ptr::null(), std::ptr::null_mut(), DIGCF_PRESENT);
        if set as isize == -1 || set as isize == 0 {
            return out;
        }
        let prop = |d: &SP_DEVINFO_DATA, p: u32| -> String {
            let mut buf = [0u16; 512];
            let mut need = 0u32;
            let ok = SetupDiGetDeviceRegistryPropertyW(
                set,
                d,
                p,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut u8,
                (buf.len() * 2) as u32,
                &mut need,
            );
            if ok == 0 {
                return String::new();
            }
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            String::from_utf16_lossy(&buf[..end])
        };
        let mut i = 0;
        loop {
            let mut d: SP_DEVINFO_DATA = std::mem::zeroed();
            d.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
            if SetupDiEnumDeviceInfo(set, i, &mut d) == 0 {
                break;
            }
            i += 1;
            let name = prop(&d, SPDRP_DEVICEDESC);
            if name.is_empty() {
                continue;
            }
            let drv = prop(&d, SPDRP_DRIVER);
            let key = format!(r"SYSTEM\CurrentControlSet\Control\Class\{drv}");
            let driver_date = registry::get_string(Hive::HKLM, &key, "DriverDate").and_then(|s| parse_driver_date(&s));
            let driver_version = registry::get_string(Hive::HKLM, &key, "DriverVersion").unwrap_or_default();
            let (real, integrated, basic_driver) = classify(&name);
            out.push(Gpu { name, driver_date, driver_version, integrated, real, basic_driver });
        }
        SetupDiDestroyDeviceInfoList(set);
    }
    out
}

/// A laptop-style setup with both an integrated and a dedicated GPU.
pub fn is_hybrid(gpus: &[Gpu]) -> bool {
    gpus.iter().any(|g| g.real && g.integrated) && gpus.iter().any(|g| g.real && !g.integrated)
}

// ------------------------------------------------------------------ drives and power

/// Some(true) if the drive is a spinning hard disk (Windows' "seek penalty" flag).
pub fn is_hard_disk(letter: char) -> Option<bool> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR,
        IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    let path = wide(&format!(r"\\.\{}:", letter.to_ascii_uppercase()));
    unsafe {
        let h = CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if h == INVALID_HANDLE_VALUE || h.is_null() {
            return None;
        }
        let mut q: STORAGE_PROPERTY_QUERY = std::mem::zeroed();
        q.PropertyId = StorageDeviceSeekPenaltyProperty;
        q.QueryType = PropertyStandardQuery;
        let mut d: DEVICE_SEEK_PENALTY_DESCRIPTOR = std::mem::zeroed();
        let mut ret = 0u32;
        let ok = DeviceIoControl(
            h,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &q as *const _ as *const _,
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            &mut d as *mut _ as *mut _,
            std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
            &mut ret,
            std::ptr::null_mut(),
        );
        CloseHandle(h);
        (ok != 0 && ret >= 8).then(|| d.IncursSeekPenalty != 0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PowerSource {
    pub has_battery: bool,
    pub on_battery: bool,
    pub battery_saver: bool,
}

pub fn power_source() -> Option<PowerSource> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    unsafe {
        let mut s: SYSTEM_POWER_STATUS = std::mem::zeroed();
        if GetSystemPowerStatus(&mut s) == 0 {
            return None;
        }
        Some(PowerSource {
            has_battery: s.BatteryFlag != 128 && s.BatteryFlag != 255,
            on_battery: s.ACLineStatus == 0,
            battery_saver: s.SystemStatusFlag == 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_numbers() {
        assert_eq!(part_number_speed("CMK16GX4M2B3200C16"), 3200);
        assert_eq!(part_number_speed("F4-3600C16-8GVKC"), 3600);
        assert_eq!(part_number_speed("TF13D416G3200HC16F"), 3200);
        assert_eq!(part_number_speed("KF432C16BB/8"), 0);
        assert_eq!(part_number_speed("M378A1K43CB2-CTD"), 0);
        assert_eq!(part_number_speed("CMH32GX5M2B6000C30"), 6000);
    }

    #[test]
    fn gpu_kinds() {
        assert_eq!(classify("NVIDIA GeForce RTX 3060 Laptop GPU"), (true, false, false));
        assert_eq!(classify("Intel(R) UHD Graphics"), (true, true, false));
        assert_eq!(classify("Intel(R) Arc(TM) A770 Graphics"), (true, false, false));
        assert_eq!(classify("Intel(R) Arc(TM) Graphics"), (true, true, false));
        assert_eq!(classify("AMD Radeon(TM) Graphics"), (true, true, false));
        assert_eq!(classify("AMD Radeon RX 6600"), (true, false, false));
        assert_eq!(classify("Microsoft Basic Display Adapter"), (false, false, true));
        assert_eq!(parse_driver_date("7-12-2024"), Some((2024, 7, 12)));
    }

    #[test]
    fn channels() {
        let m = |loc: &str, bank: &str| MemStick { locator: loc.into(), bank: bank.into(), ..Default::default() };
        assert_eq!(m("DIMM_A1", "BANK 0").channel(), Some('A'));
        assert_eq!(m("DIMM 1", "P0 CHANNEL B").channel(), Some('B'));
        assert_eq!(m("ChannelA-DIMM0", "BANK 0").channel(), Some('A'));
        assert_eq!(m("Controller0-ChannelB-DIMM0", "BANK 0").channel(), Some('B'));
        assert_eq!(m("DIMM 0", "BANK 0").channel(), None);
    }
}
