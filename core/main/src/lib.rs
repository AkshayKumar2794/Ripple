pub mod bootstrap {
    pub mod boot;
    pub mod extn;
    pub mod manifest;
    pub mod setup_extn_client_step;
    pub mod start_app_manager_step;
    pub mod start_communication_broker;
    pub mod start_fbgateway_step;
    pub mod start_ws_step;
}
pub mod broker;
pub mod firebolt {
    pub mod handlers {
        pub mod accessory_rpc;
        pub mod account_rpc;
        pub mod acknowledge_rpc;
        pub mod advertising_rpc;
        pub mod audio_description_rpc;
        pub mod authentication_rpc;
        pub mod capabilities_rpc;
        pub mod closed_captions_rpc;
        pub mod device_rpc;
        pub mod discovery_rpc;
        pub mod keyboard_rpc;
        pub mod lcm_rpc;
        pub mod lifecycle_rpc;
        pub mod localization_rpc;
        pub mod metrics_management_rpc;
        pub mod metrics_rpc;
        pub mod parameters_rpc;
        pub mod pin_rpc;
        pub mod privacy_rpc;
        pub mod profile_rpc;
        pub mod second_screen_rpc;
        pub mod secure_storage_rpc;
        pub mod user_grants_rpc;
        pub mod voice_guidance_rpc;
        pub mod wifi_rpc;
    }
    pub mod firebolt_gatekeeper;
    pub mod firebolt_gateway;
    pub mod firebolt_ws;
    pub mod rpc;
    pub mod rpc_router;
}

pub mod processor {
    pub mod account_link_processor;
    pub mod app_events_processor;
    pub mod authorized_info_processor;
    pub mod config_processor;
    pub mod exn_status_processor;
    pub mod keyboard_processor;
    pub mod lifecycle_management_processor;
    pub mod main_context_processor;
    pub mod metrics_processor;
    pub mod pin_processor;
    pub mod rpc_gateway_processor;
    pub mod settings_processor;
    pub mod storage;
    pub mod store_privacy_settings_processor;
    pub mod store_user_grants_processor;
}
pub mod state {
    pub mod bootstrap_state;
    pub mod extn_state;
    pub mod metrics_state;
    pub mod openrpc_state;
    pub mod platform_state;
    pub mod ripple_cache;
    pub mod session_state;
    pub mod cap {
        pub mod cap_state;
        pub mod generic_cap_state;
        pub mod permitted_state;
    }
}
pub mod utils;

pub mod service {
    pub mod apps;
    pub mod extn {
        pub mod ripple_client;
    }
    pub mod context_manager;
    pub mod data_governance;
    pub mod observability;
    pub mod telemetry_builder;
    pub mod user_grants;
}
