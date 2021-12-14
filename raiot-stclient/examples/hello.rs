use std::{
    sync::{
        mpsc::{channel, TryRecvError},
    },
    time::{Duration, Instant},
};

use raiot_cli::Options;
use raiot_client_base::{ConnectionSettings, D2CMsg, DMIResult};

use raiot_protocol::{direct_methods::DirectMethodReq, qos::DeliveryGuarantees};
use raiot_stclient::{conn::IotConnState, IotClient};
use serde_json::json;

fn main() -> ! {
    env_logger::init();
    let options = Options::from_cmd_line();
    let settings = options.get_connection_settings();
    let mut iot_client = connect(settings);

    let c2d_handler = |msg| println!("C2D: {}", msg);
    let c2d_hanler = Box::new(c2d_handler);
    let error_handler = Box::new(|err| println!("C2D Subscription error: {}", err));
    iot_client.sub_c2d(DeliveryGuarantees::AtMostOnce, c2d_hanler, error_handler);

    let (tx, rx) = channel();

    let dmi_handler = move |msg: DirectMethodReq| {
        println!("DMI: {}", msg);
        tx.send(msg).unwrap();
    };

    let dmi_handler = Box::new(dmi_handler);
    iot_client.sub_dmi(DeliveryGuarantees::AtMostOnce, dmi_handler);
    iot_client.sub_twin_updates(
        DeliveryGuarantees::AtMostOnce,
        Box::new(|msg| println!("Twin: {:?}", msg)),
    );
    iot_client.read_twin();

    let mut last_telemetry_time = Instant::now();
    loop {
        // send and receive messages
        iot_client.process();

        if last_telemetry_time.elapsed().as_secs() > 10 {
            let big_value = build_telemetry_msg();
            let msg = D2CMsg {
                headers: None,
                content: Some(json!({ "key": big_value })),
            };
            iot_client.send_d2c(msg, DeliveryGuarantees::AtLeastOnce);
            last_telemetry_time = Instant::now();
        }

        match rx.try_recv() {
            Ok(dmi) => {
                let res = DMIResult {
                    status: 200,
                    payload: Some(json!({ "key": "hellloooo" })),
                };
                iot_client.send_dmi_res(&dmi.request_id, res, DeliveryGuarantees::AtLeastOnce);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {}
        }

        std::thread::sleep(Duration::from_millis(5));
    }
}

fn connect(settings: ConnectionSettings) -> IotClient {
    let mut conn = IotClient::connect(&settings).unwrap();
    loop {
        match conn.complete() {
            Ok(IotConnState::Connecting(cip)) => {
                conn = cip;
                // Do some other work!
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(IotConnState::Connected(client)) => {
                println!("Connected!");
                return client;
            }
            Ok(IotConnState::ConnectFailed(rc)) => panic!("oh no! {:?}", rc),
            Err(e) => panic!("Failed connecting! {:?}", e),
        }
    }
}

fn build_telemetry_msg() -> String {
    let mut big_value = String::new();
    big_value.push('"');
    loop {
        big_value.push_str("Weee! ");
        if big_value.len() > 30000 {
            break;
        }
    }
    big_value.push('"');
    big_value
}
