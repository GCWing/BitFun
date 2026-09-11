//! Concrete MiniApp marketplace client, credential vault and package IO.

mod client;
mod package;
mod submit;

pub use crate::account_identity::{
    clear_market_credentials, load_market_credentials, save_market_credentials,
    DesktopAuthPollRequest, DesktopAuthPollResponse, DesktopAuthStart, MarketClientError, MarketMe,
    MarketTokenPair, StoredMarketCredentials,
};
pub use client::{FavoriteAggregate, MarketBrowseRequest, MarketClient, RatingAggregate};
pub use package::{
    build_market_package, validate_market_package, MarketPackageError, ValidatedMarketPackage,
};
pub use submit::{
    map_local_category_to_market, read_screenshot_file, resolve_release_target,
    submit_installed_app, suggest_market_slug, ReleaseTarget, SubmitProgress,
};
