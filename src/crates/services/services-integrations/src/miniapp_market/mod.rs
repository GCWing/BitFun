//! Concrete MiniApp marketplace client, credential vault and package IO.

mod client;
mod credentials;
mod package;

pub use client::{
    DesktopAuthPollRequest, DesktopAuthPollResponse, DesktopAuthStart, FavoriteAggregate,
    MarketBrowseRequest, MarketClient, MarketClientError, MarketMe, MarketTokenPair,
    RatingAggregate,
};
pub use credentials::{
    clear_market_credentials, load_market_credentials, save_market_credentials,
    StoredMarketCredentials,
};
pub use package::{
    build_market_package, validate_market_package, MarketPackageError, ValidatedMarketPackage,
};
