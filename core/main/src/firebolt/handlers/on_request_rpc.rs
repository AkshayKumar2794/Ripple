// Copyright 2023 Comcast Cable Communications Management, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0
//

use std::any::{Any, TypeId};

use crate::{
    firebolt::rpc::RippleRPCProvider, service::apps::provider_broker::ProviderBroker,
    state::platform_state::PlatformState,
};
use jsonrpsee::{
    core::{server::rpc_module::Methods, RpcResult},
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            provider::{
                ChallengeError, ChallengeResponse, ExternalProviderResponse, FocusRequest,
                ProviderResponse, ProviderResponsePayload, ACK_CHALLENGE_CAPABILITY,
                ACK_CHALLENGE_EVENT,
            },
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::debug,
};

macro_rules! rpc_provider_impl {
    ($name:ident, $capability:ident, $event:ident, $response_type:ty, $response_payload:expr, $error_type:ty, $error_payload:expr) => {
        #[derive(Debug)]
        pub struct $nameRPCProvider {
            pub platform_state: PlatformState,
        }

        impl $nameRPCProvider {
            async fn on_request(
                &self,
                ctx: CallContext,
                request: ListenRequest,
            ) -> RpcResult<ListenerResponse> {
                let listen = request.listen;
                debug!("on_request: request={:?}", request);
                ProviderBroker::register_or_unregister_provider(
                    &self.platform_state,
                    $capability.into(),
                    ProviderBroker::get_method($capability).unwrap_or_default(),
                    $event,
                    ctx,
                    request,
                )
                .await;

                Ok(ListenerResponse {
                    listening: listen,
                    event: $event.into(),
                })
            }

            async fn response(
                &self,
                _ctx: CallContext,
                resp: ExternalProviderResponse<$response_type>,
            ) -> RpcResult<Option<()>> {
                ProviderBroker::provider_response(
                    &self.platform_state,
                    ProviderResponse {
                        correlation_id: resp.correlation_id,
                        result: $response_payload(resp.result),
                    },
                )
                .await;
                Ok(None)
            }

            async fn error(
                &self,
                _ctx: CallContext,
                resp: ExternalProviderResponse<$error_type>,
            ) -> RpcResult<Option<()>> {
                ProviderBroker::provider_response(
                    &self.platform_state,
                    ProviderResponse {
                        correlation_id: resp.correlation_id,
                        result: $error_payload(resp.result),
                    },
                )
                .await;
                Ok(None)
            }

            async fn focus(
                &self,
                ctx: CallContext,
                request: FocusRequest,
            ) -> RpcResult<Option<()>> {
                ProviderBroker::focus(&self.platform_state, ctx, $capability.into(), request).await;
                Ok(None)
            }
        }
    };
}

pub struct OnRequestRPCProvider;

impl RippleRPCProvider<OnRequest> for OnRequestRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<OnRequest> {
        println!("*** _DEBUG: provider: entry");
        let provider_map = state.open_rpc_state.get_provider_map();
        for method in provider_map.keys() {
            if let Some(provider_set) = provider_map.get(method) {
                on_request_impl!(
                    ACK_CHALLENGE_CAPABILITY,
                    ACK_CHALLENGE_EVENT,
                    ChallengeResponse,
                    ProviderResponsePayload::ChallengeResponse,
                    ChallengeError,
                    ProviderResponsePayload::ChallengeError
                );
            }
        }

        RpcModule::new(OnRequest {
            platform_state: state.clone(),
        })
        //.register_method(method_name, callback)
    }
}

pub struct RippleRPCProviderGenerator;

impl RippleRPCProviderGenerator {
    pub fn generate(platform_state: PlatformState, mut methods: &Methods) {
        let provider_map = state.open_rpc_state.get_provider_map();
        for method in provider_map.keys() {
            if let Some(provider_set) = provider_map.get(method) {
                rpc_provider_impl!(
                    ACK_CHALLENGE_NAME,
                    ACK_CHALLENGE_CAPABILITY,
                    ACK_CHALLENGE_EVENT,
                    ChallengeResponse,
                    ProviderResponsePayload::ChallengeResponse,
                    ChallengeError,
                    ProviderResponsePayload::ChallengeError
                );
            }
        }
    }
}
