//! Display refresh-rate inspection: catches the classic "144 Hz monitor running at 60 Hz".

use super::{from_wide, wide};
use windows_sys::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, EnumDisplayDevicesW, EnumDisplaySettingsW, CDS_UPDATEREGISTRY,
    DEVMODEW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISP_CHANGE_SUCCESSFUL,
    DM_DISPLAYFREQUENCY, ENUM_CURRENT_SETTINGS,
};

#[derive(Clone, Debug)]
pub struct DisplayInfo {
    pub device: String,
    pub monitor: String,
    pub width: u32,
    pub height: u32,
    pub current_hz: u32,
    pub max_hz: u32,
}

fn devmode() -> DEVMODEW {
    let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
    dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    dm
}

pub fn displays() -> Vec<DisplayInfo> {
    let mut out = Vec::new();
    let mut i = 0u32;
    loop {
        let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        if unsafe { EnumDisplayDevicesW(std::ptr::null(), i, &mut dd, 0) } == 0 {
            break;
        }
        i += 1;
        if dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0 {
            continue;
        }
        let device = from_wide(&dd.DeviceName);
        let dev_w = wide(&device);

        let mut cur = devmode();
        if unsafe { EnumDisplaySettingsW(dev_w.as_ptr(), ENUM_CURRENT_SETTINGS, &mut cur) } == 0 {
            continue;
        }
        let mut max_hz = cur.dmDisplayFrequency;
        let mut m = 0u32;
        loop {
            let mut dm = devmode();
            if unsafe { EnumDisplaySettingsW(dev_w.as_ptr(), m, &mut dm) } == 0 {
                break;
            }
            m += 1;
            if dm.dmPelsWidth == cur.dmPelsWidth && dm.dmPelsHeight == cur.dmPelsHeight {
                max_hz = max_hz.max(dm.dmDisplayFrequency);
            }
        }

        let mut mon: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        mon.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let monitor = if unsafe { EnumDisplayDevicesW(dev_w.as_ptr(), 0, &mut mon, 0) } != 0 {
            from_wide(&mon.DeviceString)
        } else {
            device.clone()
        };

        out.push(DisplayInfo {
            device,
            monitor,
            width: cur.dmPelsWidth,
            height: cur.dmPelsHeight,
            current_hz: cur.dmDisplayFrequency,
            max_hz,
        });
    }
    out
}

pub fn set_refresh_rate(device: &str, hz: u32) -> Result<(), String> {
    let dev_w = wide(device);
    let mut dm = devmode();
    if unsafe { EnumDisplaySettingsW(dev_w.as_ptr(), ENUM_CURRENT_SETTINGS, &mut dm) } == 0 {
        return Err("could not read current display mode".into());
    }
    dm.dmDisplayFrequency = hz;
    dm.dmFields = DM_DISPLAYFREQUENCY;
    let rc = unsafe {
        ChangeDisplaySettingsExW(dev_w.as_ptr(), &dm, std::ptr::null_mut(), CDS_UPDATEREGISTRY, std::ptr::null())
    };
    if rc == DISP_CHANGE_SUCCESSFUL {
        Ok(())
    } else {
        Err(format!("display driver refused the change (code {rc})"))
    }
}
