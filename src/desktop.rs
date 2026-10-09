//! Documented Windows controls. No power-limit, voltage, firmware or kernel-driver access.
use crate::settings::PowerPlan;
use std::ptr;
use windows_sys::{
    Win32::{
        Foundation::LocalFree,
        System::Power::{PowerGetActiveScheme, PowerSetActiveScheme},
        UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SYSCOMMAND,
        },
    },
    core::GUID,
};

pub fn active_power() -> Option<PowerPlan> {
    let mut id = ptr::null_mut();
    // SAFETY: API allocates a GUID on success; copy its value and free it exactly once.
    unsafe {
        if PowerGetActiveScheme(ptr::null_mut(), &mut id) != 0 || id.is_null() {
            return None;
        }
        let value = *id;
        LocalFree(id.cast());
        [
            PowerPlan::Balanced,
            PowerPlan::PowerSaver,
            PowerPlan::HighPerformance,
        ]
        .into_iter()
        .find(|p| {
            let expected = power_id(*p);
            expected.data1 == value.data1
                && expected.data2 == value.data2
                && expected.data3 == value.data3
                && expected.data4 == value.data4
        })
    }
}
fn power_id(plan: PowerPlan) -> GUID {
    GUID::from_u128(match plan {
        PowerPlan::Balanced => 0x381b4222_f694_41f0_9685_ff5bb260df2e,
        PowerPlan::PowerSaver => 0xa1841308_3541_4fab_bc81_f71556f20b4a,
        PowerPlan::HighPerformance => 0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c,
    })
}

pub fn power(plan: PowerPlan) -> Result<(), String> {
    let id = power_id(plan);
    // SAFETY: Standard system scheme GUID; null root selects current user's policy store.
    let code = unsafe { PowerSetActiveScheme(ptr::null_mut(), &id) };
    if code != 0 {
        return Err(format!(
            "Power plan unavailable (Windows status {code}); no replacement plan is created"
        ));
    }
    if active_power() != Some(plan) {
        return Err(
            "Windows did not confirm the requested power plan; current settings were refreshed"
                .into(),
        );
    }
    Ok(())
}
pub fn screen_off() -> Result<(), String> {
    let mut reply = 0;
    // SAFETY: Documented SC_MONITORPOWER message with no pointer payload; bounded timeout.
    let result = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SYSCOMMAND,
            0xf170,
            2,
            SMTO_ABORTIFHUNG,
            1000,
            &mut reply,
        )
    };
    if result == 0 {
        Err("Display power message was not acknowledged".into())
    } else {
        Ok(())
    }
}
