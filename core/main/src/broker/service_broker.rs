use ripple_sdk::{
    api::gateway::rpc_gateway_api::{JsonRpcApiError, JsonRpcApiResponse},
    log::{error, info},
    tokio::sync::mpsc,
};
use ssda_types::gateway::{ServiceRoutingRequest, ServiceRoutingResponse};
use ssda_types::ServiceRequestId;

use crate::state::platform_state::PlatformState;

use super::endpoint_broker::{
    BrokerCallback, BrokerCleaner, BrokerConnectRequest, BrokerOutputForwarder, BrokerRequest,
    BrokerSender, EndpointBroker, EndpointBrokerState, BROKER_CHANNEL_BUFFER_SIZE,
};
use ripple_sdk::tokio;

pub struct ServiceBroker {
    platform_state: Option<PlatformState>,
    connect_request: BrokerConnectRequest,
    broker_callback: BrokerCallback,
    endpoint_broker_state: EndpointBrokerState,
    broker_sender: BrokerSender,
    cleaner: BrokerCleaner,
}
async fn send_broker_response(callback: &BrokerCallback, request: &BrokerRequest, body: &[u8]) {
    match BrokerOutputForwarder::handle_non_jsonrpc_response(
        body,
        callback.clone(),
        request.clone(),
    ) {
        Ok(_) => {}
        Err(e) => {
            error!("Error message from http broker {:?}", e)
        }
    }
}

impl ServiceBroker {}
impl EndpointBroker for ServiceBroker {
    fn get_broker(
        ps: Option<crate::state::platform_state::PlatformState>,
        connect_request: super::endpoint_broker::BrokerConnectRequest,
        broker_callback: super::endpoint_broker::BrokerCallback,
        endpoint_broker: &mut super::endpoint_broker::EndpointBrokerState,
    ) -> Self {
        //todo!();
        // let endpoint = request.endpoint.clone();
        let (tx, mut tr) = mpsc::channel(BROKER_CHANNEL_BUFFER_SIZE);
        let broker_sender = BrokerSender { sender: tx };
        let callback = broker_callback.clone();
        if let Some(platform_state) = ps.clone() {
            tokio::spawn(async move {
                while let Some(request) = tr.recv().await {
                    info!("ServiceBroker received request: {:?}", request);
                    let services_tx = platform_state
                        .services_gateway_api
                        .lock()
                        .await
                        .get_sender();
                    use tokio::sync::oneshot;

                    let (oneshot_tx, oneshot_rx) = oneshot::channel::<ServiceRoutingResponse>();
                    let service_request = ServiceRoutingRequest {
                        request_id: ServiceRequestId {
                            request_id: request.rpc.ctx.call_id,
                        },
                        payload: request.rpc.clone(),
                        respond_to: oneshot_tx,
                    };

                    services_tx.try_send(service_request).unwrap();

                    match oneshot_rx.await {
                        Ok(response) => {
                            info!("ServiceBroker received response: {:?}", response);
                            match response {
                                ServiceRoutingResponse::Error(e) => {
                                    error!("ServiceBroker received error response: {:?}", e);
                                    let err = JsonRpcApiError::default()
                                        .with_id(e.request_id.request_id)
                                        .with_message(e.error)
                                        .to_response();
                                    send_broker_response(&callback, &request, &err.as_bytes())
                                        .await;
                                }
                                ServiceRoutingResponse::Success(response) => {
                                    info!(
                                        "ServiceBroker received success response: {:?}",
                                        response
                                    );
                                    let win = JsonRpcApiResponse::default()
                                        .with_id(response.request_id.request_id)
                                        .with_result(Some(response.response))
                                        .as_bytes();
                                    send_broker_response(&callback, &request, &win).await;
                                }
                            }
                        }
                        Err(e) => {
                            error!("ServiceBroker failed to receive response {}", e);
                        }
                    }
                }
            });
        } else {
            panic!("Platform state is required");
        };

        Self {
            platform_state: ps.clone(),
            connect_request: connect_request,
            broker_callback: broker_callback,
            endpoint_broker_state: endpoint_broker.clone(),
            broker_sender: broker_sender,
            cleaner: BrokerCleaner { cleaner: None },
        }
    }

    fn get_sender(&self) -> super::endpoint_broker::BrokerSender {
        self.broker_sender.clone()
    }

    fn get_cleaner(&self) -> super::endpoint_broker::BrokerCleaner {
        BrokerCleaner::default()
    }
}
