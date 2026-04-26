//! Internationalization (i18n) service implementation
//!
//! Provides backend text translation.

use fluent_bundle::concurrent::FluentBundle as ConcurrentFluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue as FV};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;
use unic_langid::LanguageIdentifier;

use super::locale_registry::LOCALE_REGISTRY;
use super::types::{FluentValue, LocaleId, LocaleMetadata, TranslationArgs};
use crate::service::config::ConfigService;
use crate::util::errors::*;

/// Type alias for a thread-safe `FluentBundle`.
type ConcurrentBundle = ConcurrentFluentBundle<FluentResource>;

/// I18n service
pub struct I18nService {
    /// Current locale
    current_locale: Arc<RwLock<LocaleId>>,
    /// Locale bundle collection (using the thread-safe `FluentBundle`)
    bundles: Arc<RwLock<HashMap<LocaleId, ConcurrentBundle>>>,
    /// Config service
    config_service: Option<Arc<ConfigService>>,
    /// Whether the service has been initialized
    initialized: Arc<RwLock<bool>>,
}

impl I18nService {
    /// Creates a new i18n service.
    pub fn new() -> Self {
        Self {
            current_locale: Arc::new(RwLock::new(LocaleId::default())),
            bundles: Arc::new(RwLock::new(HashMap::new())),
            config_service: None,
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Creates an i18n service with a config service.
    pub fn with_config_service(config_service: Arc<ConfigService>) -> Self {
        Self {
            current_locale: Arc::new(RwLock::new(LocaleId::default())),
            bundles: Arc::new(RwLock::new(HashMap::new())),
            config_service: Some(config_service),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Initializes the i18n service.
    pub async fn initialize(&self) -> BitFunResult<()> {
        let mut initialized = self.initialized.write().await;
        if *initialized {
            warn!("I18nService already initialized");
            return Ok(());
        }

        info!("Initializing i18n service");

        self.load_all_bundles().await?;

        if let Some(ref config_service) = self.config_service {
            // Prefer `app.language` (desktop source of truth), then legacy `i18n.currentLanguage`.
            let mut resolved: Option<LocaleId> = None;
            if let Ok(app_lang) = config_service
                .get_config::<String>(Some("app.language"))
                .await
            {
                resolved = LocaleId::from_str(&app_lang);
            }
            if resolved.is_none() {
                if let Ok(locale) = config_service
                    .get_config::<LocaleId>(Some("i18n.currentLanguage"))
                    .await
                {
                    resolved = Some(locale);
                }
            }
            match resolved {
                Some(locale) => {
                    let mut current = self.current_locale.write().await;
                    *current = locale;
                    info!("Loaded locale from config: {}", current.as_str());
                }
                None => {
                    debug!("Locale config not found, using default");
                }
            }
        }

        *initialized = true;
        info!("I18n service initialized");
        Ok(())
    }

    /// Loads all locale bundles.
    async fn load_all_bundles(&self) -> BitFunResult<()> {
        let mut bundles = self.bundles.write().await;

        for locale in LOCALE_REGISTRY {
            if let Some(bundle) = Self::create_bundle(locale.code, locale.fluent_source) {
                bundles.insert(locale.id, bundle);
            }
        }

        info!("Loaded {} locale bundle(s)", bundles.len());
        Ok(())
    }

    /// Creates a locale bundle (thread-safe version).
    fn create_bundle(locale_str: &str, ftl_content: &str) -> Option<ConcurrentBundle> {
        let langid: LanguageIdentifier = locale_str.parse().ok()?;
        let mut bundle = ConcurrentFluentBundle::new_concurrent(vec![langid]);

        let resource = FluentResource::try_new(ftl_content.to_string()).ok()?;
        bundle.add_resource(resource).ok()?;

        Some(bundle)
    }

    /// Returns the current locale.
    pub async fn get_current_locale(&self) -> LocaleId {
        *self.current_locale.read().await
    }

    /// Sets the current in-memory locale.
    ///
    /// Persistence is owned by the caller through `app.language`, which is the
    /// canonical cross-runtime source of truth. Keeping this method memory-only
    /// avoids reviving `i18n.currentLanguage` writes on every runtime switch.
    pub async fn set_locale(&self, locale: LocaleId) -> BitFunResult<()> {
        let old_locale = {
            let mut current = self.current_locale.write().await;
            let old = *current;
            *current = locale;
            old
        };

        info!(
            "Locale changed: {} -> {}",
            old_locale.as_str(),
            locale.as_str()
        );
        Ok(())
    }

    /// Returns all supported locales.
    pub fn get_supported_locales(&self) -> Vec<LocaleMetadata> {
        LocaleMetadata::all()
    }

    /// Translates text.
    pub async fn translate(&self, key: &str, args: Option<TranslationArgs>) -> String {
        let locale = *self.current_locale.read().await;
        self.translate_with_locale(&locale, key, args).await
    }

    /// Translates text with a specific locale.
    pub async fn translate_with_locale(
        &self,
        locale: &LocaleId,
        key: &str,
        args: Option<TranslationArgs>,
    ) -> String {
        let bundles = self.bundles.read().await;

        if let Some(bundle) = bundles.get(locale) {
            if let Some(result) = Self::format_message(bundle, key, args.as_ref()) {
                return result;
            }
        }

        if locale != &LocaleId::EnUS {
            if let Some(bundle) = bundles.get(&LocaleId::EnUS) {
                if let Some(result) = Self::format_message(bundle, key, args.as_ref()) {
                    return result;
                }
            }
        }

        key.to_string()
    }

    /// Formats a message.
    fn format_message(
        bundle: &ConcurrentBundle,
        key: &str,
        args: Option<&TranslationArgs>,
    ) -> Option<String> {
        let msg = bundle.get_message(key)?;
        let pattern = msg.value()?;

        let mut errors = vec![];

        let fluent_args: Option<FluentArgs> = args.map(|a| {
            let mut fa = FluentArgs::new();
            for (k, v) in a.iter() {
                match v {
                    FluentValue::String(s) => {
                        fa.set(k.clone(), FV::from(s.clone()));
                    }
                    FluentValue::Number(n) => {
                        fa.set(k.clone(), FV::from(*n));
                    }
                }
            }
            fa
        });

        let result = bundle.format_pattern(pattern, fluent_args.as_ref(), &mut errors);

        if !errors.is_empty() {
            warn!(
                "Translation formatting warning for key '{}': {:?}",
                key, errors
            );
        }

        Some(result.to_string())
    }

    /// Convenience translation (no args).
    pub async fn t(&self, key: &str) -> String {
        self.translate(key, None).await
    }

    /// Convenience translation (with args).
    pub async fn t_with(&self, key: &str, args: TranslationArgs) -> String {
        self.translate(key, Some(args)).await
    }

    /// Returns whether the service has been initialized.
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }
}

impl Default for I18nService {
    fn default() -> Self {
        Self::new()
    }
}

// Global singleton (optional)
static GLOBAL_I18N_SERVICE: LazyLock<Arc<RwLock<Option<Arc<I18nService>>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(None)));

/// Gets the global i18n service.
pub async fn get_global_i18n_service() -> Option<Arc<I18nService>> {
    GLOBAL_I18N_SERVICE.read().await.clone()
}

/// Updates the global i18n service locale if it has been initialized.
pub async fn sync_global_i18n_service_locale(locale: LocaleId) -> BitFunResult<bool> {
    if let Some(service) = get_global_i18n_service().await {
        service.set_locale(locale).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Sets the global i18n service.
pub async fn set_global_i18n_service(service: Arc<I18nService>) {
    let mut global = GLOBAL_I18N_SERVICE.write().await;
    *global = Some(service);
}

/// Initializes the global i18n service.
pub async fn initialize_global_i18n_service(
    config_service: Option<Arc<ConfigService>>,
) -> BitFunResult<Arc<I18nService>> {
    let service = match config_service {
        Some(cs) => Arc::new(I18nService::with_config_service(cs)),
        None => Arc::new(I18nService::new()),
    };

    service.initialize().await?;
    set_global_i18n_service(service.clone()).await;

    Ok(service)
}
