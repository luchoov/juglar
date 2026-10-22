//! The "Show animations in Windows" setting (Settings > Accessibility >
//! Visual effects), Windows' equivalent of a reduced-motion preference.

use windows::core::BOOL;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

/// `true` when the user turned animations off. A failed query counts as on,
/// since Windows shows animations by default.
pub fn reduced_motion() -> bool {
    let mut animations = BOOL(1);
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the pointer,
    // which points at a live local of that type.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&raw mut animations).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_ok() && !animations.as_bool()
}
