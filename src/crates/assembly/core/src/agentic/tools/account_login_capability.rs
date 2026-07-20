//! Account login gate for tools that require a BitFun account session.

use std::sync::atomic::{AtomicBool, Ordering};

static ACCOUNT_LOGIN_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark whether the current process has a fully logged-in BitFun account session.
pub fn set_account_login_available(available: bool) {
    ACCOUNT_LOGIN_AVAILABLE.store(available, Ordering::SeqCst);
}

pub fn account_login_available() -> bool {
    ACCOUNT_LOGIN_AVAILABLE.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_account_login_availability() {
        set_account_login_available(false);
        assert!(!account_login_available());
        set_account_login_available(true);
        assert!(account_login_available());
        set_account_login_available(false);
        assert!(!account_login_available());
    }
}
