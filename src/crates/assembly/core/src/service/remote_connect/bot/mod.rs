//! Bot integration compatibility facade for Remote Connect.
//!
//! Provider-neutral bot DTOs, menu and locale rendering, persistence, and file
//! delivery helpers live in `bitfun-services-integrations`. Core keeps the
//! command router and platform bot adapters because they still call concrete
//! session, coordinator, image, and runtime services.

pub mod command_router;
pub mod feishu;
pub mod locale;
pub mod menu;
pub mod telegram;
pub mod weixin;

pub use bitfun_services_integrations::remote_connect::bot::{
    auto_push_failed_message, auto_push_intro, auto_push_skip_too_large_message,
    collect_auto_push_files, detect_mime_type, extract_computer_file_paths,
    extract_downloadable_file_paths, format_file_size, get_file_metadata, load_bot_persistence,
    read_workspace_file, resolve_workspace_path, save_bot_persistence, AutoPushFile, BotConfig,
    BotLanguage, BotPairingInfo, BotPersistenceData, MenuItem, MenuItemStyle, MenuView,
    RemoteConnectFormState, RemoteDeviceTarget, SavedBotConnection, WorkspaceFileContent,
};
pub use command_router::{
    set_delegated_identity_provider, BotChatState, ForwardRequest, ForwardedTurnResult,
    HandleResult,
};
