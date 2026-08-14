use agent_client_protocol::{Builder, HandleDispatchFrom};
use bitfun_app_server_protocol::account::*;

use super::capability::unsupported_management_handler;
use crate::management::{ACCOUNT_CAPABILITY, SETTINGS_SYNC_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder() -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("account and settings sync handlers")
        .on_receive_request(
            unsupported_management_handler!(ACCOUNT_CAPABILITY, AccountSnapshotRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(ACCOUNT_CAPABILITY, AccountLoginRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(ACCOUNT_CAPABILITY, AccountFinalizeLoginRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(ACCOUNT_CAPABILITY, AccountLogoutRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(SETTINGS_SYNC_CAPABILITY, SettingsSyncStartRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(SETTINGS_SYNC_CAPABILITY, SettingsSyncSnapshotRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(SETTINGS_SYNC_CAPABILITY, SettingsSyncCancelRequest),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            unsupported_management_handler!(
                SETTINGS_SYNC_CAPABILITY,
                SettingsSyncLocalChangedRequest
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
