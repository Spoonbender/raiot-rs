use chrono::prelude::*;
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use sha2::Sha256;
use std::error::Error;
use std::time::Duration;
use url::form_urlencoded::byte_serialize;

// TODO proper URL encoding of device and module IDs
// TODO expose Result<,> and get rid of unwraps

type TokenResult = Result<SasToken, Box<dyn Error>>;

/// Represents a single SAS token of a device or module
#[derive(Clone, Debug)]
pub struct SasToken {
    value: String,
}

impl SasToken {
    /// Generates a SAS token for a device connection
    pub fn for_device(server_addr: &str, device_id: &str, key: &str, ttl: Duration) -> TokenResult {
        assert!(ttl.as_secs() > 0);
        let encoded_device_id = utf8_percent_encode(&device_id, NON_ALPHANUMERIC).to_string();
        let resource_uri = format!("{}/devices/{}", &server_addr, &encoded_device_id);
        get_sas_token(&key, &resource_uri, ttl)
    }

    /// Generates a SAS token for a device module connection
    pub fn for_module(
        server_addr: &str,
        device_id: &str,
        module_id: &str,
        key: &str,
        ttl: Duration,
    ) -> TokenResult {
        assert!(ttl.as_secs() > 0);

        let encoded_device_id = utf8_percent_encode(&device_id, NON_ALPHANUMERIC).to_string();
        let encoded_module_id = utf8_percent_encode(&module_id, NON_ALPHANUMERIC).to_string();
        let resource_uri = format!(
            "{}/devices/{}/modules/{}",
            &server_addr, &encoded_device_id, &encoded_module_id
        );
        get_sas_token(&key, &resource_uri, ttl)
    }
}

impl From<SasToken> for String {
    fn from(token: SasToken) -> Self {
        token.value
    }
}

fn get_sas_token(key: &str, resource_uri: &str, ttl: Duration) -> TokenResult {
    type HmacSha256 = Hmac<Sha256>;
    let key = base64::decode(key)?;
    let expiry: DateTime<Utc> = Utc::now() + chrono::Duration::from_std(ttl).unwrap();
    let encoded_uri: String = byte_serialize(resource_uri.as_bytes()).collect();
    let string_to_sign = format!("{}\n{}", encoded_uri, &expiry.timestamp().to_string());
    let mut mac = HmacSha256::new_varkey(&key).expect("HMAC can take key of any size");
    mac.input(string_to_sign.as_bytes());
    let hash = mac.result().code();
    let signature = base64::encode(&hash);
    let encoded_signature: String = byte_serialize(signature.as_bytes()).collect();
    let token = format!(
        "SharedAccessSignature sr={}&sig={}&se={}",
        encoded_uri,
        encoded_signature,
        expiry.timestamp()
    );
    return Ok(SasToken { value: token });
}
