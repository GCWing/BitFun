//! Desktop-only gate for Computer use (set from BitFun desktop at startup).

use std::sync::atomic::{AtomicBool, Ordering};

static COMPUTER_USE_DESKTOP_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark whether this process is BitFun desktop with OS automation wired up.
pub fn set_computer_use_desktop_available(available: bool) {
    COMPUTER_USE_DESKTOP_AVAILABLE.store(available, Ordering::SeqCst);
}

pub fn computer_use_desktop_available() -> bool {
    COMPUTER_USE_DESKTOP_AVAILABLE.load(Ordering::SeqCst)
}
