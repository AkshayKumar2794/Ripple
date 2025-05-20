use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use gateway::ServiceRoutingRequest;
use ripple_sdk::api::rules_engine::{Rule, RuleTransform};
use ripple_sdk::log::{debug, error, info};
use serde::{Deserialize, Serialize};

use service::{
    APIClientMessages, APIGatewayServiceRegistrationRequest, ServiceRegistration,
    ServiceRequestHandler,
};

/*
Who what why
# API Gateway
The API gateway is a standalone binary component
that listens for Firebolt requests on a websocket. Upon receiving a firebolt
request, the gateway will look up a handler service (based on the method name) in it's
runtime (dynamically created) registry, and (assumign a service is registered to for the method),
will wrap the request with metadata, and dispatch the request to the handler service. Upon receiving
a response from the service, the API Gateway will translate the service response (using the rules engine)
into a Firebolt compatible (success or failure) result.

# Servicesprintln
ServiceRequestHandler. This design is motivated by the need to free the Service from as much connection oriented
detail and let the developer focus on business logic. The API Gateway client is instanced as a crate that can be
consumed by a service at the highest possible level of abstraction (and ease of use) - it should only requuire a bit of
bootstrapping, and a ServiceRequestHandler instance/implementation, and then it should manage all the details of the
interactions between the Service and the API gateway with the Service being as blissfully ignorant of the details
as possible.
*/
/*

register: Service -> API Gateway Client -> API Gateway

*/

pub mod api_gateway_client;
pub mod api_gateway_server;
pub mod service_api;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]

pub struct JqRule {
    pub alias: String,
    pub rule: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StaticRule {
    pub alias: String,
    pub rule: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]

pub struct HandlerId {
    pub handler_id: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]

pub struct ServiceHandler {
    pub handler_id: HandlerId,
    pub handler_type: Handler,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Handler {
    #[default]
    None,
    JqRule(JqRule),
    StaticRule(StaticRule),
}
impl std::fmt::Display for Handler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Handler::None => write!(f, "none"),
            Handler::JqRule(jq_rule) => write!(f, "{}", jq_rule.alias),
            Handler::StaticRule(static_rule) => write!(f, "{}", static_rule.alias),
        }
    }
}

impl From<Handler> for ripple_sdk::api::rules_engine::Rule {
    fn from(handler: Handler) -> Self {
        match handler {
            Handler::None => Rule {
                alias: "none".to_string(),
                filter: None,
                transform: RuleTransform::default(),
                event_handler: None,
                endpoint: None,
                sources: None,
            },
            Handler::JqRule(jq_rule) => Rule {
                alias: jq_rule.alias,
                filter: Some(jq_rule.rule),
                ..Default::default()
            },
            Handler::StaticRule(static_rule) => Rule {
                alias: static_rule.alias,
                filter: Some(static_rule.rule),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceId {
    pub service_id: String,
}
impl ServiceId {
    pub fn new(service_id: String) -> Self {
        ServiceId { service_id }
    }
}

impl PartialEq for ServiceId {
    fn eq(&self, other: &Self) -> bool {
        self.service_id == other.service_id
    }
}

impl Eq for ServiceId {}

impl std::hash::Hash for ServiceId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.service_id.hash(state);
    }
}
impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.service_id)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceRequestId {
    pub request_id: u64,
}
impl PartialEq for ServiceRequestId {
    fn eq(&self, other: &Self) -> bool {
        self.request_id == other.request_id
    }
}

impl Eq for ServiceRequestId {}

impl std::hash::Hash for ServiceRequestId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.request_id.hash(state);
    }
}
impl ServiceRequestId {
    pub fn new(request_id: u64) -> Self {
        ServiceRequestId { request_id }
    }
}

/*
gateway messages: from endpoint broker to api gateway and back.
*/
pub mod gateway {
    use crate::ServiceId;
    use http::Uri;
    use ripple_sdk::api::gateway::rpc_gateway_api::{
        JsonRpcApiError, JsonRpcApiResponse, RpcRequest,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use tokio::sync::oneshot::Sender;

    use crate::ServiceRequestId;

    /*
    ServiceRequest is the request that is sent from the API Gateway to the service
    */
    #[derive(Debug)]
    pub struct ServiceRoutingRequest {
        pub request_id: ServiceRequestId,
        pub payload: RpcRequest,
        pub respond_to: Sender<ServiceRoutingResponse>,
    }
    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct ServiceRoutingSuccessResponse {
        pub request_id: ServiceRequestId,
        pub response: Value,
    }
    impl From<ServiceRoutingResponse> for JsonRpcApiResponse {
        fn from(response: ServiceRoutingResponse) -> Self {
            match response {
                ServiceRoutingResponse::Error(_error) => JsonRpcApiError::default().into(),
                ServiceRoutingResponse::Success(success) => JsonRpcApiResponse {
                    id: Some(success.request_id.request_id),
                    jsonrpc: "2.0".to_string(),
                    result: Some(success.response),
                    error: None,
                    method: None,
                    params: None,
                },
            }
        }
    }
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceRoutingErrorResponse {
        pub request_id: ServiceRequestId,
        pub error: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum ServiceRoutingResponse {
        Success(ServiceRoutingSuccessResponse),
        Error(ServiceRoutingErrorResponse),
    }
    #[derive(Debug)]
    pub enum APIGatewayServiceConnectionDisposition {
        Accept(ServiceId),
        Connected(ServiceId),
    }
    #[derive(Debug)]
    pub enum APIGatewayServiceConnectionError {
        ConnectionError,
        NotAService,
    }

    /*
    This is the API gateway, and it meant to be hosted in the main ripple process
    */

    #[async_trait::async_trait]
    pub trait ApiGatewayServer: Send + Sync {
        async fn is_service_connect(
            &self,
            uri: Uri,
        ) -> Result<APIGatewayServiceConnectionDisposition, APIGatewayServiceConnectionError>;
        async fn service_connect(
            &mut self,
            service_id: ServiceId,
            ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        ) -> Result<APIGatewayServiceConnectionDisposition, APIGatewayServiceConnectionError>;
        fn get_sender(&self) -> tokio::sync::mpsc::Sender<ServiceRoutingRequest>;
    }
}
/*
service message: from api gateawy to services and back (over websockets)
*/
pub mod service {

    use serde::{Deserialize, Serialize};

    use crate::{Handler, JqRule, ServiceId, ServiceRequestId};

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct FireboltMethodHandlerRegistration {
        pub firebolt_method: Handler,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct FireboltMethodHandlerAPIRegistration {
        pub firebolt_method: String,
        pub jq_rule: Option<JqRule>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct APIGatewayServiceRegistrationRequest {
        pub firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct APIGatewayServiceRegistrationResponse {
        pub firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
    }
    impl From<APIGatewayServiceRegistrationRequest> for APIGatewayServiceRegistrationResponse {
        fn from(
            request: APIGatewayServiceRegistrationRequest,
        ) -> APIGatewayServiceRegistrationResponse {
            APIGatewayServiceRegistrationResponse {
                firebolt_handlers: request.firebolt_handlers,
            }
        }
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceRequest {
        pub service_id: ServiceId,
        pub firebolt_method: Handler,
        pub payload: serde_json::Value,
    }
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceErrorResponse {
        pub service_id: ServiceId,
        pub firebolt_method: Handler,
        pub error: String,
    }
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceSuccessResponse {
        pub service_id: ServiceId,
        pub firebolt_method: Handler,
        pub payload: serde_json::Value,
    }

    /*
    send by api client to the api gateway over websocket

    */
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct APIClientRegistration {
        pub firebolt_handlers: Vec<FireboltMethodHandlerRegistration>,
    }
    /*
    Sent during callback registration in service
    */
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServiceRegistration {
        pub service_id: ServiceId,
        pub firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
    }

    impl ServiceRegistration {
        pub fn new(
            service_id: ServiceId,
            firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
        ) -> Self {
            ServiceRegistration {
                service_id,
                firebolt_handlers,
            }
        }
        pub fn get_rule_registrations(&self) -> Vec<FireboltMethodHandlerAPIRegistration> {
            self.firebolt_handlers.clone()
        }
    }

    pub struct ServiceRegistrationBuilder {
        service_id: ServiceId,
        firebolt_handlers: Vec<FireboltMethodHandlerAPIRegistration>,
    }
    impl ServiceRegistrationBuilder {
        pub fn new(service_id: ServiceId) -> Self {
            ServiceRegistrationBuilder {
                service_id,
                firebolt_handlers: Vec::new(),
            }
        }
        pub fn add_handler(&mut self, firebolt_method: Handler) -> &mut Self {
            self.firebolt_handlers.push(match firebolt_method {
                Handler::None => todo!(),
                Handler::JqRule(jq_rule) => FireboltMethodHandlerAPIRegistration {
                    firebolt_method: jq_rule.alias.clone(),
                    jq_rule: Some(jq_rule),
                },
                Handler::StaticRule(static_rule) => FireboltMethodHandlerAPIRegistration {
                    firebolt_method: static_rule.alias.clone(),
                    jq_rule: Some(JqRule {
                        alias: static_rule.alias,
                        rule: static_rule.rule,
                    }),
                },
            });
            self
        }
        pub fn build(&self) -> ServiceRegistration {
            ServiceRegistration {
                service_id: self.service_id.clone(),
                firebolt_handlers: self.firebolt_handlers.clone(),
            }
        }
    }
    /*
    ServiceCall is the request that is presented to a callback handler.
    */
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceCall {
        pub request_id: ServiceRequestId,
        pub method: String,
        pub payload: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceCallSuccessResponse {
        pub request_id: ServiceRequestId,
        pub response: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceCallErrorResponse {
        pub request_id: ServiceRequestId,
        pub error: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum ServiceCallResponse {
        Success(ServiceCallSuccessResponse),
        Error(ServiceCallErrorResponse),
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum APIClientMessages {
        Register(APIGatewayServiceRegistrationRequest),
        Registered(APIGatewayServiceRegistrationResponse),
        Error(String),
        Unregister(ServiceId),
        ServiceCall(ServiceCall),
        ServiceCallSuccessResponse(ServiceCallSuccessResponse),
        ServiceCallErrorResponse(ServiceCallErrorResponse),
    }
    impl Default for APIClientMessages {
        fn default() -> Self {
            APIClientMessages::Register(APIGatewayServiceRegistrationRequest::default())
        }
    }

    pub struct ServiceRegistrationResponse {
        pub service_id: ServiceId,
    }
    pub struct ServiceRegistrationFailure {
        pub service_id: ServiceId,
        pub error: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum WebsocketServiceResponse {
        Success(ServiceRequestId, serde_json::Value),
        Error(ServiceRequestId, String),
    }
    impl WebsocketServiceResponse {
        pub fn get_id(&self) -> ServiceRequestId {
            match self {
                WebsocketServiceResponse::Success(id, _) => id.clone(),
                WebsocketServiceResponse::Error(id, _) => id.clone(),
            }
        }
    }
    #[derive(Debug)]
    pub struct WebsocketServiceRequest {
        pub request_id: ServiceRequestId,
        pub method: String,
        pub payload: serde_json::Value,
        pub respond_to: tokio::sync::oneshot::Sender<WebsocketServiceResponse>,
    }

    /*
    individiual services implement this trait to handle requests from the API gateway
    This trait should faciliate fun testing of actual implementations
    */

    pub trait ServiceRequestHandler: Send + Sync {
        /*
          called by the client to allow the ServiceRequestHander return a Vec<FireboltMethodHandler>
          to be used by the client to route requests. This is a blocking call, and will be called
          after the client `on_connect`s
        **/
        fn register(&self) -> Vec<FireboltMethodHandlerRegistration>;
        fn handle_request(
            &self,
            request: ServiceCall,
        ) -> Result<ServiceCallSuccessResponse, ServiceCallErrorResponse>;
        fn on_connected(&self);
        fn on_disconnected(&self);
        fn healthy(&self) -> bool;
    }
}
pub mod client {
    use mockall::automock;

    use crate::{HandlerId, ServiceId};

    use crate::service::{
        FireboltMethodHandlerRegistration, ServiceCall, ServiceCallErrorResponse,
        ServiceCallSuccessResponse, ServiceErrorResponse, ServiceRegistration,
        ServiceRegistrationFailure, ServiceRegistrationResponse, ServiceRequest,
        ServiceRequestHandler, ServiceSuccessResponse,
    };

    #[async_trait::async_trait]
    /*
    this is a trait that is implemented by a service client.
    There will probably only be 2 of these for the foreseeable future:
    1) a real, websocket client
    2) a mock, for unit/integration testing
    the point of this trait , and it's implementers, is to abstract the details related to auth, connection, etc. from the service (business logic)
    that needs it.

    the methods in the actual interface are concerned with what to do during lifecycle transition events. the graph/steps/etc. of the lifecyle and
    when to call these methods (and any state needed to call them) is the responsiblity of the concrete implementation.
    */
    #[automock]
    pub trait ServiceClientTrait: Send + Sync {
        // this is a request to the service to register itself with the API gateway
        fn register(
            &self,
            registraton: Box<ServiceRegistration>,
        ) -> Result<ServiceRegistrationResponse, ServiceRegistrationFailure>;
        fn set_handler(&mut self, handler: Box<dyn ServiceRequestHandler>);
        fn unregister_service(&mut self, service_id: ServiceId) -> Result<(), String>;
        fn register_handler(
            &mut self,
            handler: FireboltMethodHandlerRegistration,
        ) -> Result<(), String>;
        fn unregister_handler(&mut self, handler_id: HandlerId) -> Result<(), String>;
        async fn invoke_handler(
            &mut self,
            request: ServiceRequest,
        ) -> Result<ServiceSuccessResponse, ServiceErrorResponse>;
    }
    impl<T> ServiceRequestHandler for T
    where
        T: Send + Sync + Clone + 'static + ServiceRequestHandlerImpl,
    {
        fn register(&self) -> Vec<FireboltMethodHandlerRegistration> {
            self.register()
        }

        fn handle_request(
            &self,
            request: ServiceCall,
        ) -> Result<ServiceCallSuccessResponse, ServiceCallErrorResponse> {
            self.handle_request(request)
        }

        fn on_connected(&self) {
            self.on_connected()
        }

        fn on_disconnected(&self) {
            self.on_disconnected()
        }

        fn healthy(&self) -> bool {
            self.healthy()
        }
    }

    pub trait ServiceRequestHandlerImpl: Send + Sync + Clone {
        fn register(&self) -> Vec<FireboltMethodHandlerRegistration>;
        fn handle_request(
            &self,
            request: ServiceCall,
        ) -> Result<ServiceCallSuccessResponse, ServiceCallErrorResponse>;
        fn on_connected(&self);
        fn on_disconnected(&self);
        fn healthy(&self) -> bool;
    }
}
/*
This is the alternative API surface that services can use to communicate with the API gateway.
Firebolt calls are bidirectional, so the API gateway can send messages to the service, and the service can send messages to the API gateway.
*/
mod api_surface {
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FireboltRequest {
        pub request_id: String,
        pub payload: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FireboltResponse {
        pub request_id: String,
        pub payload: String,
    }
}

/*
This trait represents the layer that will actually "talk" to the API gateway. it handles transports, etc.
This is a trait to :
1) Enable testing
2) Abstract the details of the transport from the service
3) Allow for different transports (websocket, http, etc.)z
*/
#[async_trait::async_trait]
pub trait APIGatewayClient {
    fn connect(&self);
    fn disconnect(&self);
    fn dispatch(&self, request: gateway::ServiceRoutingRequest);
    async fn start(&self);
    async fn stop(&self);
}
pub struct WebsocketAPIGatewayClient {
    handler: Arc<dyn service::ServiceRequestHandler>,
    registration: ServiceRegistration,
}

use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

impl WebsocketAPIGatewayClient {
    pub fn new(handler: Arc<dyn ServiceRequestHandler>, registration: ServiceRegistration) -> Self {
        WebsocketAPIGatewayClient {
            handler,
            registration,
        }
    }
    pub async fn handle_messages(
        stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        handler: Arc<dyn ServiceRequestHandler>,
        registration: ServiceRegistration,
    ) {
        let (mut tx, mut rx) = stream.split();

        let registration_request = APIGatewayServiceRegistrationRequest {
            firebolt_handlers: registration.get_rule_registrations(),
        };
        let msg = serde_json::to_string(&APIClientMessages::Register(registration_request.clone()));
        if let Ok(msg) = msg {
            let _ = tx.send(Message::Text(msg)).await;
        } else {
            error!("websocket: Failed to serialize registration request");
        }

        while let Some(message) = rx.next().await {
            match message {
                Ok(msg) => {
                    /*
                    attempt to marshal the message into a ServiceRequest
                    if it fails, log the error and continue
                    */
                    info!("websocket: Received message via websocket: {:?}", msg);
                    let msg = msg.into_text().unwrap_or_default();
                    if let Ok(request) = serde_json::from_str::<APIClientMessages>(&msg) {
                        match request {
                            APIClientMessages::Register(registration_request) => {
                                info!("Received registration request: {:?}", registration_request);
                            }
                            APIClientMessages::Registered(registration_request) => {
                                info!("Received registered response: {:?}", registration_request);
                                handler.on_connected();
                            }
                            APIClientMessages::Error(error) => {
                                info!("Received error: {:?}", error);
                            }
                            APIClientMessages::Unregister(service_id) => {
                                info!("Received unregister request: {:?}", service_id);
                            }
                            APIClientMessages::ServiceCall(service_call) => {
                                info!("Received service call: {:?}", service_call);
                                match handler.handle_request(service_call) {
                                    Ok(response) => {
                                        let response =
                                            APIClientMessages::ServiceCallSuccessResponse(response);
                                        // Send the response back to the client
                                        let response = serde_json::to_string(&response);
                                        if let Ok(response) = response {
                                            let _ = tx.send(Message::Text(response.clone())).await;
                                            debug!("Sending response: {:?}", response);
                                        } else {
                                            error!("Failed to serialize response");
                                        }
                                    }
                                    Err(e) => {
                                        // Handle error response
                                        error!("Error handling request: {:?}", e);
                                        let error_response =
                                            APIClientMessages::ServiceCallErrorResponse(e);
                                        let error_response = serde_json::to_string(&error_response);
                                        if let Ok(error_response) = error_response {
                                            let _ = tx
                                                .send(Message::Text(error_response.clone()))
                                                .await;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else {
                        error!(" I don't understand this message: {:?}", msg);
                    }
                }
                Err(err) => {
                    error!("Error receiving message: {:?}", err);
                    // Handle the error (e.g., log it, retry, etc.)
                    // You might want to break the loop or handle reconnection logic here
                    // break;
                }
            }
        }
    }
    pub async fn connect(
        &self,
        endpoint_url: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = Url::parse(
            &endpoint_url
                .unwrap_or_else(|| "ws://localhost:3474/apigateway?serviceId=tester".to_string()),
        )?;
        let mut backoff = Duration::from_secs(1);

        loop {
            match connect_async(url.clone()).await {
                Ok((ws_stream, _)) => {
                    let handler = self.handler.clone();
                    let registration = self.registration.clone();

                    info!("✅ Connected to WebSocket");

                    // Reset backoff after successful connection
                    backoff = Duration::from_secs(1);

                    // This handles the message loop and returns on disconnect
                    Self::handle_messages(ws_stream, handler, registration).await;
                    info!("🔌 Disconnected, retrying...");
                }
                Err(err) => {
                    error!("Error connecting to WebSocket: {:?}", err);

                    // Exponential backoff (up to 1 minute)
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, Duration::from_secs(60));
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl APIGatewayClient for WebsocketAPIGatewayClient {
    fn connect(&self) {
        // connect to the API gateway
    }
    fn disconnect(&self) {
        // disconnect from the API gateway
    }
    fn dispatch(&self, _request: ServiceRoutingRequest) {
        // send a request to the API gateway
    }

    async fn start(&self) {
        info!("starting websocket client");
        self.connect(None).await.unwrap();
        self.handler.on_connected();
    }

    async fn stop(&self) {
        todo!()
    }
}

pub struct DBUSAPIGatewayClient {
    // dbus client
}
#[async_trait::async_trait]
impl APIGatewayClient for DBUSAPIGatewayClient {
    fn connect(&self) {
        // connect to the API gateway
    }
    fn disconnect(&self) {
        // disconnect from the API gateway
    }
    fn dispatch(&self, _request: ServiceRoutingRequest) {
        // send a request to the API gateway
    }

    async fn start(&self) {
        todo!()
    }

    async fn stop(&self) {
        todo!()
    }
}
