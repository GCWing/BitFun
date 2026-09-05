use std::sync::Arc;

use agent_client_protocol::{Builder, HandleDispatchFrom};
use openbitfun_app_server_protocol::account::*;

use super::capability::management_handler;
use crate::management::{AppManagementService, ACCOUNT_CAPABILITY, SETTINGS_SYNC_CAPABILITY};
use crate::role::{AppClient, AppServer};

pub(in crate::server) fn builder(
    management: Option<Arc<AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("account and settings sync handlers")
        .on_receive_request(
            management_handler!(
                management,
                ACCOUNT_CAPABILITY,
                AccountSnapshotRequest,
                account_snapshot
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                ACCOUNT_CAPABILITY,
                AccountLoginRequest,
                account_login
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                ACCOUNT_CAPABILITY,
                AccountFinalizeLoginRequest,
                account_finalize_login
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                ACCOUNT_CAPABILITY,
                AccountLogoutRequest,
                account_logout
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                SETTINGS_SYNC_CAPABILITY,
                SettingsSyncStartRequest,
                settings_sync_start
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                SETTINGS_SYNC_CAPABILITY,
                SettingsSyncSnapshotRequest,
                settings_sync_snapshot
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                SETTINGS_SYNC_CAPABILITY,
                SettingsSyncCancelRequest,
                settings_sync_cancel
            ),
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            management_handler!(
                management,
                SETTINGS_SYNC_CAPABILITY,
                SettingsSyncLocalChangedRequest,
                settings_sync_local_changed
            ),
            agent_client_protocol::on_receive_request!(),
        )
}
