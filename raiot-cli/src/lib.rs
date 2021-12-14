use std::time::Duration;

use raiot_client_base::ConnectionSettings;
use raiot_protocol::{
    auth::{certificate::DeviceCertificate, DeviceCredentials},
    qos::SessionMode,
    ClientIdentity,
};
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Options {
    #[structopt(short = "p", long = "port", default_value = "8883")]
    pub port: u16,

    #[structopt(short = "h", long = "hostname")]
    pub hostname: String,

    #[structopt(short = "d", long = "device")]
    pub device_id: String,

    #[structopt(short = "k", long = "key")]
    pub key: Option<String>,

    #[structopt(long = "cert-file")]
    pub cert_file: Option<String>,

    #[structopt(long = "cert-pass")]
    pub cert_pass: Option<String>,

    #[structopt(long = "connect-timeout", default_value = "30")]
    pub connect_timeout_secs: u16,

    #[structopt(long = "token-ttl", default_value = "60")]
    pub token_ttl_mins: u64,
}

impl Options {
    pub fn from_cmd_line() -> Options {
        Options::from_args()
    }

    pub fn get_connection_settings(&self) -> ConnectionSettings {
        ConnectionSettings {
            hostname: self.hostname.clone(),
            client_id: ClientIdentity::from_device_id(&self.device_id),
            port: self.port,
            timeout: Duration::from_secs(self.connect_timeout_secs as u64),
            session_mode: SessionMode::Clean,
            token_ttl: Duration::from_secs(60 * self.token_ttl_mins),
            credentials: self.get_credentials(),
        }
    }

    pub fn get_credentials(&self) -> DeviceCredentials {
        if let Some(ref key) = self.key {
            DeviceCredentials::Sas(key.clone())
        } else if self.cert_file.is_some() && self.cert_pass.is_some() {
            DeviceCredentials::Certificate(DeviceCertificate {
                bytes: std::fs::read(std::path::PathBuf::from(&self.cert_file.as_ref().unwrap()))
                    .unwrap(),
                password: self.cert_pass.as_ref().unwrap().clone(),
            })
        } else {
            panic!("Must provide certificate + password, or SAS key");
        }
    }
}
