//! Account login gate for tools that require a BitFun account session.

use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

static ACCOUNT_LOGIN_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark whether the current process has a fully logged-in BitFun account session.
pub fn set_account_login_available(available: bool) {
    ACCOUNT_LOGIN_AVAILABLE.store(available, Ordering::SeqCst);
}

pub fn account_login_available() -> bool {
    ACCOUNT_LOGIN_AVAILABLE.load(Ordering::SeqCst)
}

#[cfg(test)]
static ACCOUNT_LOGIN_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) struct AccountLoginTestGuard {
    _guard: MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for AccountLoginTestGuard {
    fn drop(&mut self) {
        set_account_login_available(false);
    }
}

#[cfg(test)]
pub(crate) fn lock_account_login_for_test() -> AccountLoginTestGuard {
    let guard = ACCOUNT_LOGIN_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    set_account_login_available(false);
    AccountLoginTestGuard { _guard: guard }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_account_login_availability() {
        let _guard = lock_account_login_for_test();
        set_account_login_available(false);
        assert!(!account_login_available());
        set_account_login_available(true);
        assert!(account_login_available());
        set_account_login_available(false);
        assert!(!account_login_available());
    }
}
