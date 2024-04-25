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

use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::apps::provider_broker::ProviderBroker,
    state::{openrpc_state::ProviderSet, platform_state::PlatformState},
};
use jsonrpsee::{
    core::{server::rpc_module::Methods, Error, RpcResult},
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_pin::PinChallengeResponse,
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

use paste::paste;

macro_rules! rpc_provider_impl {
    ($name:ident, $response_type:ty, $response_payload:expr, $error_type:ty, $error_payload:expr) => {
        paste! {
            #[derive(Debug)]
            pub struct [<$name RPCProvider>] {
                pub platform_state: PlatformState,
                capability: String,
                event: &'static str,
            }

            impl [<$name RPCProvider>] {

                pub fn new(platform_state: PlatformState, capability: String, event: &'static str) -> [<$name RPCProvider>] {
                    [<$name RPCProvider>] {
                        platform_state,
                        capability,
                        event
                    }
                }

                async fn on_request(
                    &self,
                    ctx: CallContext,
                    request: ListenRequest,
                ) -> RpcResult<ListenerResponse> {
                    let listen = request.listen;
                    debug!("on_request: request={:?}", request);
                    ProviderBroker::register_or_unregister_provider(
                        &self.platform_state,
                        self.capability.clone(),
                        ProviderBroker::get_method(&self.capability).unwrap_or_default(),
                        &self.event,
                        ctx,
                        request,
                    )
                    .await;

                    Ok(ListenerResponse {
                        listening: listen,
                        event: String::from(self.event),
                    })
                }

                async fn response(
                    &self,
                    _ctx: CallContext,
                    resp: ExternalProviderResponse<$response_type>,
                    //resp: ExternalProviderResponse<[<$response_type>]>,
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
                    ProviderBroker::focus(&self.platform_state, ctx, self.capability.clone(), request).await;
                    Ok(None)
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct RPCProvider {
    pub platform_state: PlatformState,
    capability: String,
    event: &'static str,
}

impl RPCProvider {
    pub fn new(
        platform_state: PlatformState,
        capability: String,
        event: &'static str,
    ) -> RPCProvider {
        RPCProvider {
            platform_state,
            capability,
            event,
        }
    }

    pub fn foo() {
        println!("*** _DEBUG: foo: entry");
    }

    fn get_response_payload<T>(&self, result: T) -> Option<ProviderResponsePayload> {
        if self.capability.eq(ACK_CHALLENGE_CAPABILITY) {
            Some(ProviderResponsePayload::ChallengeResponse(result))
        } else {
            None
        }
    }

    fn get_error_payload<U>(&self, result: U) -> Option<ProviderResponsePayload> {
        if self.capability.eq(ACK_CHALLENGE_CAPABILITY) {
            Some(ProviderResponsePayload::ChallengeError(result))
        } else {
            None
        }
    }

    async fn on_request(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        debug!("on_request: request={:?}", request);
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            self.capability.clone(),
            ProviderBroker::get_method(&self.capability).unwrap_or_default(),
            self.event,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening: listen,
            event: self.event.into(),
        })
    }

    async fn response<T>(
        &self,
        _ctx: CallContext,
        resp: ExternalProviderResponse<T>,
    ) -> RpcResult<Option<()>> {
        if let Some(payload) = self.get_response_payload(resp.result) {
            ProviderBroker::provider_response(
                &self.platform_state,
                ProviderResponse {
                    correlation_id: resp.correlation_id,
                    result: payload,
                },
            )
            .await;
            return Ok(None);
        }
        Err(Error::Custom(String::from("No response payload defined")))
    }

    async fn error<T>(
        &self,
        _ctx: CallContext,
        resp: ExternalProviderResponse<T>,
    ) -> RpcResult<Option<()>> {
        if let Some(error_payload) = self.get_error_payload(resp.result) {
            ProviderBroker::provider_response(
                &self.platform_state,
                ProviderResponse {
                    correlation_id: resp.correlation_id,
                    result: error_payload,
                },
            )
            .await;
            return Ok(None);
        }
        Err(Error::Custom(String::from(
            "No error response payload defined",
        )))
    }

    async fn focus(&self, ctx: CallContext, request: FocusRequest) -> RpcResult<Option<()>> {
        ProviderBroker::focus(&self.platform_state, ctx, self.capability.clone(), request).await;
        Ok(None)
    }
}

// pub struct OnRequestRPCProvider;

// impl RippleRPCProvider<OnRequest> for OnRequestRPCProvider {
//     fn provide(state: PlatformState) -> RpcModule<OnRequest> {
//         println!("*** _DEBUG: provider: entry");
//         let provider_map = state.open_rpc_state.get_provider_map();
//         for method in provider_map.keys() {
//             if let Some(provider_set) = provider_map.get(method) {
//                 rpc_provider_impl!(
//                     ACK_CHALLENGE_CAPABILITY,
//                     ACK_CHALLENGE_EVENT,
//                     ChallengeResponse,
//                     ProviderResponsePayload::ChallengeResponse,
//                     ChallengeError,
//                     ProviderResponsePayload::ChallengeError
//                 );
//             }
//         }

//         RpcModule::new(OnRequest {
//             platform_state: state.clone(),
//         })
//         //.register_method(method_name, callback)
//     }
// }

pub struct RippleRPCProviderGenerator;

impl RippleRPCProviderGenerator {
    // pub fn generate(platform_state: &PlatformState, mut methods: &Methods) {
    //     let provider_map = state.open_rpc_state.get_provider_map();
    //     for method in provider_map.keys() {
    //         if let Some(provider_set) = provider_map.get(method) {
    //             rpc_provider_impl!(
    //                 ACK_CHALLENGE_NAME,
    //                 ACK_CHALLENGE_CAPABILITY,
    //                 ACK_CHALLENGE_EVENT,
    //                 ChallengeResponse,
    //                 ProviderResponsePayload::ChallengeResponse,
    //                 ChallengeError,
    //                 ProviderResponsePayload::ChallengeError
    //             );
    //         }
    //     }
    // }
    pub fn generate(platform_state: &PlatformState, mut methods: &Methods) {
        let provider_map: HashMap<String, ProviderSet> =
            platform_state.open_rpc_state.get_provider_map();
        for method in provider_map.keys() {
            if let Some(provider_set) = provider_map.get(method) {
                if let Some(attributes) = provider_set.attributes.clone() {
                    let name = attributes.name;
                    let provides = provider_set.provides.as_ref().unwrap().as_str();
                    let event = attributes.event;
                    let response_type = attributes.response_type;
                    let response_payload = attributes.response_payload;
                    let error_type = attributes.error_type;
                    let error_payload = attributes.error_payload;

                    match provides.as_str() {
                        "xrn:firebolt:capability:usergrant:acknowledgechallenge" => {
                            rpc_provider_impl!(
                                name,
                                ChallengeResponse,
                                ProviderResponsePayload::ChallengeResponse,
                                ChallengeError,
                                ProviderResponsePayload::ChallengeError
                            );
                        }
                        "bar" => {
                            rpc_provider_impl!(
                                name,
                                PinChallengeResponse,
                                ProviderResponsePayload::PinChallengeResponse,
                                ChallengeError,
                                ProviderResponsePayload::ChallengeError
                            );
                        }
                        _ => println!("Unhandled"),
                    }

                    // if name.eq("foo") {
                    //     rpc_provider_impl!(
                    //         name,
                    //         //response_type,
                    //         ChallengeResponse,
                    //         //response_payload,
                    //         ProviderResponsePayload::ChallengeResponse,
                    //         //error_type,
                    //         ChallengeError,
                    //         //error_payload
                    //         ProviderResponsePayload::ChallengeError
                    //     );
                    // } else if name.eq("bar") {
                    //     rpc_provider_impl!(
                    //         name,
                    //         //response_type,
                    //         PinChallengeResponse,
                    //         //response_payload,
                    //         ProviderResponsePayload::PinChallengeResponse,
                    //         //error_type,
                    //         ChallengeError,
                    //         //error_payload
                    //         ProviderResponsePayload::ChallengeError
                    //     );
                    // }

                    // let provider = RPCProvider::new(platform_state.clone(), provides, event);
                    // let module = RpcModule::new(());
                    // module.register_async_method("foo", |_, _| provider.foo());
                }
            }
        }
    }
}
