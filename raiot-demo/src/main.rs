#[macro_use] extern crate log;

use raiot_client_base::ConnectionSettings;
use raiot_cli::Options;
use raiot_protocol::*;

use structopt::StructOpt;

use std::{time::{Instant, Duration}};
use serde_json::json;
use raiot_client::dmi::*;
use raiot_client::c2d::*;
use raiot_client::d2c::D2CMsg;
use raiot_protocol::auth::{DeviceCredentials, certificate::DeviceCertificate};
use qos::{SessionMode, DeliveryGuarantees};



#[tokio::main]
pub async fn main() {

    env_logger::init();
    debug!("Starting IoT Hub Device");

    let options = Options::from_args();
    debug!("Connecting to {}:{}", options.hostname, options.port);
    let credentials: DeviceCredentials;
    if let Some(key) = options.key {
        credentials = DeviceCredentials::Sas(key)
    } else {
        if options.cert_file.is_some() &&
           options.cert_pass.is_some() {
            credentials = DeviceCredentials::Certificate(DeviceCertificate {
                bytes: std::fs::read(std::path::PathBuf::from(options.cert_file.unwrap())).unwrap(),
                password: options.cert_pass.unwrap()
            })
           } else {
               panic!("Must provide certificate + password, or SAS key");
           }
    }
    let settings = ConnectionSettings {
        hostname: options.hostname,
        client_id: ClientIdentity::from_device_id(&options.device_id),
        port: options.port,
        timeout: Duration::from_secs(30),
        session_mode: SessionMode::Clean,
        token_ttl: Duration::from_secs(60 * 60 * 24),
        credentials: credentials
    };

    let socket = raiot_client::iot_socket::IotSocket::connect(settings);
    
    debug!("Got socket");

    let mut client = raiot_client::DeviceClient::new(ClientIdentity::from_device_id(&options.device_id), socket);
 
    debug!("Reading the twin...");
    let twin = client.read_twin().await;
    debug!("Got the twin: {:?}", twin);

    client.set_dmi_handler(handle_direct_method, DeliveryGuarantees::AtMostOnce);
    client.set_c2d_handler(handle_c2d, DeliveryGuarantees::AtMostOnce);

    let mut last_telemetry_instant = Instant::now();
    let tx_freq= Duration::from_secs(3);

    loop {
        if last_telemetry_instant.elapsed() > tx_freq {
            client.send_telemetry(D2CMsg {
                content: Some(json!({
                    "hello" : "world"
                })),
                headers: None
            }).await.unwrap();
            last_telemetry_instant = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}

fn handle_direct_method(req: DMIRequest) -> DMIResult {
    debug!("Got DMI request: {:?}", req);
    DMIResult {
        status: 200,
        payload: Some(json!({ "key" : "value" })),
    }
}

fn handle_c2d(msg: C2DMsg) -> C2DResult {
    debug!("Got C2D Msg: {:?}", msg);
    Ok(())
}
