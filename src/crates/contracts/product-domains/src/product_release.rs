//! Stable OpenBitFun product-release facts shared across product domains.

use semver::Version;

/// OpenBitFun begins as a new product at this release.
pub const OPENBITFUN_INITIAL_RELEASE_VERSION: Version = Version::new(1, 0, 0);

#[cfg(test)]
mod tests {
    use super::OPENBITFUN_INITIAL_RELEASE_VERSION;

    #[test]
    fn initial_release_is_stable_one_zero_zero() {
        assert_eq!(OPENBITFUN_INITIAL_RELEASE_VERSION.to_string(), "1.0.0");
        assert!(OPENBITFUN_INITIAL_RELEASE_VERSION.pre.is_empty());
        assert!(OPENBITFUN_INITIAL_RELEASE_VERSION.build.is_empty());
    }
}
