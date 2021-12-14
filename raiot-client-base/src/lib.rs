use std::{collections::HashMap, time::Duration};

use raiot_protocol::{
    auth::sas::SasToken, auth::DeviceCredentials, qos::PacketId, qos::SessionMode, ClientIdentity,
};

#[derive(Clone, Debug)]
pub struct ConnectionSettings {
    pub hostname: String,
    pub port: u16,
    pub client_id: ClientIdentity,
    pub session_mode: SessionMode,
    pub timeout: Duration,
    pub token_ttl: Duration,
    pub credentials: DeviceCredentials,
}

pub fn generate_sas_token(settings: &ConnectionSettings, key: &str) -> SasToken {
    match &settings.client_id {
        ClientIdentity::Device(device) => SasToken::for_device(
            &settings.hostname,
            &device.device_id,
            key,
            settings.token_ttl,
        )
        .expect("Token expected to be valid"),
        ClientIdentity::Module(module) => SasToken::for_module(
            &settings.hostname,
            &module.device_id,
            &module.module_id,
            key,
            settings.token_ttl,
        )
        .expect("Token expected to be valid"),
    }
}

#[derive(Debug, Clone)]
pub struct D2CMsg {
    pub content: Option<serde_json::Value>,
    pub headers: Option<HashMap<String, String>>,
}

pub trait DeviceClient {
    fn send_d2c(msg: D2CMsg);
}

#[derive(Debug, Clone)]
pub struct DMIRequest {
    pub method_name: String,
    pub body: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct DMIResult {
    pub status: i32,
    pub payload: Option<serde_json::Value>,
}

type DMICompletion = fn(String, DMIResult) -> ();

pub struct DMIResponder {
    /// Invocation request ID
    request_id: String,

    response_fn: DMICompletion,
}

impl DMIResponder {
    pub fn complete(self, result: DMIResult) {
        let op: DMICompletion = self.response_fn;
        op(self.request_id, result)
    }
}

pub type DMIHandler = fn(DMIRequest, DMIResponder) -> DMIResult;

pub struct PacketsNumerator {
    value: u16,
}

impl PacketsNumerator {
    pub fn new() -> PacketsNumerator {
        PacketsNumerator { value: 0 }
    }

    pub fn next(&mut self) -> PacketId {
        self.value += 1;
        self.value.into()
    }
}
