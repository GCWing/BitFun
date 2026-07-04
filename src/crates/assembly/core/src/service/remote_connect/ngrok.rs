//! Compatibility facade for Remote Connect ngrok tunnel lifecycle.

pub use bitfun_services_integrations::remote_connect::{
    cleanup_all_ngrok, detect_running_ngrok, is_ngrok_available, start_ngrok_tunnel, NgrokTunnel,
};
