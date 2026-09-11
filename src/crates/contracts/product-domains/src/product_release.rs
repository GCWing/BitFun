//! Stable OpenBitFun product-release facts shared across product domains.

use semver::Version;

/// OpenBitFun begins as a new product at this release.
pub const OPENBITFUN_INITIAL_RELEASE_VERSION: Version = Version::new(1, 0, 0);

/// Market packages use stable product compatibility versions. The inaugural
/// public release is named `1.0.0-beta`, but supports the 1.0.0 market contract.
/// Keep this exception separate from updater SemVer ordering and from numbered
/// beta / release-candidate builds, which retain normal prerelease semantics.
pub fn supports_market_minimum_version(current: &Version, minimum: &Version) -> bool {
    current >= minimum
        || (current.major == 1
            && current.minor == 0
            && current.patch == 0
            && current.pre.as_str() == "beta"
            && minimum == &OPENBITFUN_INITIAL_RELEASE_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inaugural_beta_supports_stable_market_contract_only() {
        let initial = &OPENBITFUN_INITIAL_RELEASE_VERSION;
        let beta = Version::parse("1.0.0-beta").unwrap();
        assert!(supports_market_minimum_version(&beta, initial));
        assert!(!supports_market_minimum_version(
            &beta,
            &Version::new(1, 0, 1)
        ));
        for version in ["0.2.19", "1.0.0-beta.1", "1.0.0-rc.1"] {
            assert!(!supports_market_minimum_version(
                &Version::parse(version).unwrap(),
                initial
            ));
        }
        assert!(supports_market_minimum_version(
            &Version::new(1, 0, 0),
            initial
        ));
        assert!(supports_market_minimum_version(
            &Version::new(1, 1, 0),
            initial
        ));
    }

    #[test]
    fn initial_release_is_stable_one_zero_zero() {
        assert_eq!(OPENBITFUN_INITIAL_RELEASE_VERSION.to_string(), "1.0.0");
        assert!(OPENBITFUN_INITIAL_RELEASE_VERSION.pre.is_empty());
        assert!(OPENBITFUN_INITIAL_RELEASE_VERSION.build.is_empty());
    }
}
