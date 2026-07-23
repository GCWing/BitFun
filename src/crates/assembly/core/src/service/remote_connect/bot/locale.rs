pub use bitfun_services_integrations::remote_connect::bot::{
    fmt_count, strings_for, BotLanguage, BotStrings,
};

pub async fn current_bot_language() -> BotLanguage {
    match crate::service::config::get_app_language().await {
        crate::service::LocaleId::ZhCN => BotLanguage::ZhCN,
        crate::service::LocaleId::ZhTW => BotLanguage::ZhTW,
        crate::service::LocaleId::EnUS => BotLanguage::EnUS,
    }
}
