use std::sync::Arc;

use agent_client_protocol::schema::{
    AuthenticateRequest, AuthenticateResponse, CancelNotification, InitializeRequest,
    InitializeResponse, ListSessionsRequest, ListSessionsResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
    SetSessionModeResponse, SetSessionModelRequest, SetSessionModelResponse,
};
use agent_client_protocol::{
    Agent, ByteStreams, Client, ConnectTo, ConnectionTo, Dispatch, Error, Result,
};
use async_trait::async_trait;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

/// Runtime operations needed by the ACP protocol layer.
#[async_trait]
pub trait AcpRuntime: Send + Sync + 'static {
    async fn initialize(&self, request: InitializeRequest) -> Result<InitializeResponse>;

    async fn authenticate(&self, _request: AuthenticateRequest) -> Result<AuthenticateResponse> {
        Ok(AuthenticateResponse::new())
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
        connection: ConnectionTo<Client>,
    ) -> Result<NewSessionResponse>;

    async fn load_session(
        &self,
        _request: LoadSessionRequest,
        _connection: ConnectionTo<Client>,
    ) -> Result<LoadSessionResponse> {
        Err(Error::method_not_found().data("session/load is not implemented"))
    }

    async fn list_sessions(&self, request: ListSessionsRequest) -> Result<ListSessionsResponse>;

    async fn prompt(&self, request: PromptRequest) -> Result<PromptResponse>;

    async fn cancel(&self, notification: CancelNotification) -> Result<()>;

    async fn set_session_mode(
        &self,
        _request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse> {
        Err(Error::method_not_found().data("session/set_mode is not implemented"))
    }

    async fn set_session_config_option(
        &self,
        _request: SetSessionConfigOptionRequest,
    ) -> Result<SetSessionConfigOptionResponse> {
        Err(Error::method_not_found().data("session/set_config_option is not implemented"))
    }

    async fn set_session_model(
        &self,
        _request: SetSessionModelRequest,
    ) -> Result<SetSessionModelResponse> {
        Err(Error::method_not_found().data("session/set_model is not implemented"))
    }
}

/// Typed ACP server backed by an injected BitFun runtime.
pub struct AcpServer<R> {
    runtime: Arc<R>,
}

impl<R> AcpServer<R>
where
    R: AcpRuntime,
{
    pub fn new(runtime: Arc<R>) -> Self {
        Self { runtime }
    }

    pub async fn serve_stdio(self) -> Result<()> {
        let stdin = tokio::io::stdin().compat();
        let stdout = tokio::io::stdout().compat_write();
        self.serve(ByteStreams::new(stdout, stdin)).await
    }

    pub async fn serve(self, transport: impl ConnectTo<Agent> + 'static) -> Result<()> {
        let runtime = self.runtime;

        Agent
            .builder()
            .name("bitfun-acp")
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: InitializeRequest, responder, _cx| {
                        responder.respond_with_result(runtime.initialize(request).await)
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: AuthenticateRequest,
                                responder,
                                cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        cx.spawn(async move {
                            responder.respond_with_result(runtime.authenticate(request).await)
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: NewSessionRequest, responder, cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        let session_cx = cx.clone();
                        cx.spawn(async move {
                            responder
                                .respond_with_result(runtime.new_session(request, session_cx).await)
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: LoadSessionRequest, responder, cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        let session_cx = cx.clone();
                        cx.spawn(async move {
                            responder.respond_with_result(
                                runtime.load_session(request, session_cx).await,
                            )
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: ListSessionsRequest,
                                responder,
                                cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        cx.spawn(async move {
                            responder.respond_with_result(runtime.list_sessions(request).await)
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: PromptRequest, responder, cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        cx.spawn(async move {
                            responder.respond_with_result(runtime.prompt(request).await)
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                {
                    let runtime = runtime.clone();
                    async move |notification: CancelNotification, cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        cx.spawn(async move {
                            if let Err(error) = runtime.cancel(notification).await {
                                log::error!("Error handling ACP cancel notification: {:?}", error);
                            }
                            Ok(())
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: SetSessionModeRequest,
                                responder,
                                cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        cx.spawn(async move {
                            responder.respond_with_result(runtime.set_session_mode(request).await)
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: SetSessionConfigOptionRequest,
                                responder,
                                cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        cx.spawn(async move {
                            responder.respond_with_result(
                                runtime.set_session_config_option(request).await,
                            )
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let runtime = runtime.clone();
                    async move |request: SetSessionModelRequest,
                                responder,
                                cx: ConnectionTo<Client>| {
                        let runtime = runtime.clone();
                        cx.spawn(async move {
                            responder.respond_with_result(runtime.set_session_model(request).await)
                        })?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_dispatch(
                async move |message: Dispatch, cx: ConnectionTo<Client>| {
                    message.respond_with_error(Error::method_not_found(), cx)
                },
                agent_client_protocol::on_receive_dispatch!(),
            )
            .connect_to(transport)
            .await
    }
}
