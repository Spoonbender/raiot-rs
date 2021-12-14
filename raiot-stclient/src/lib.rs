#[macro_use]
extern crate log;

pub mod conn;
mod sub;

use raiot_client_base::{D2CMsg, DMIResult, PacketsNumerator};
use raiot_protocol::{
    c2d::C2DMsg,
    twin::{DesiredPropsUpdated, ReadTwinRes, TwinUpdatesSub},
};
use raiot_protocol::{direct_methods::DirectMethodReq, MsgFromHub};
use raiot_protocol::{direct_methods::DirectMethodRes, SubRes};
use raiot_protocol::{direct_methods::DirectMethodsSub, twin::TwinReadSub};
use std::{net::TcpStream, time::Duration};
use sub::{SubErrorHandler, SubState};

use native_tls::TlsStream;
use raiot_mqtt::connection::MqttConnection;
use raiot_protocol::{
    c2d::C2DSub, qos::DeliveryGuarantees,
    telemetry::TelemetryMsg, twin::ReadTwinReq, ClientIdentity, IotCodec,
};

pub type C2DHandler = dyn Fn(C2DMsg);
pub type DMIHandler = dyn Fn(DirectMethodReq);
pub type TwinUpdatesHandler = dyn Fn(DesiredPropsUpdated);
pub type TwinReadsHandler = dyn Fn(ReadTwinRes);

type MyStream = TlsStream<TcpStream>;

pub struct IotClient {
    connection: MqttConnection<MyStream>,
    client_id: ClientIdentity,
    packets_numerator: PacketsNumerator,
    #[cfg(feature = "twin")]
    twin_read: SubState<ReadTwinRes>,
    #[cfg(feature = "direct-methods")]
    dmi: SubState<DirectMethodReq>,
    #[cfg(feature = "twin")]
    twin_updates: SubState<DesiredPropsUpdated>,
    #[cfg(feature = "c2d")]
    c2d: SubState<C2DMsg>,
}

impl IotClient {
    pub fn send_d2c(&mut self, msg: D2CMsg, mode: DeliveryGuarantees) {
        let msg = TelemetryMsg {
            client_id: self.client_id.clone(), // TODO
            content: msg.content,
            headers: msg.headers,
            packet_id: match mode {
                DeliveryGuarantees::AtMostOnce => None,
                DeliveryGuarantees::AtLeastOnce => Some(self.packets_numerator.next()),
            },
        };
        let msg = IotCodec::encode_message(&msg.into()).unwrap();
        self.connection.write(&msg).unwrap();
    }

    pub fn sub_dmi(&mut self, mode: DeliveryGuarantees, handler: Box<DMIHandler>) {
        let packet_id = self.packets_numerator.next();
        let msg = DirectMethodsSub { mode, packet_id };
        let msg = IotCodec::encode_message(&msg.into()).unwrap();
        self.dmi = SubState::Subscribing(handler, Box::new(|e| println!("DMI Sub Error: {}", e)), packet_id);
        self.connection.write(&msg).unwrap();
    }

    pub fn send_dmi_res(&mut self, request_id: &str, res: DMIResult, mode: DeliveryGuarantees) {
        let msg = DirectMethodRes {
            request_id: request_id.to_owned(),
            status: res.status,
            payload: res.payload,
            packet_id: match mode {
                DeliveryGuarantees::AtMostOnce => None,
                DeliveryGuarantees::AtLeastOnce => Some(self.packets_numerator.next()),
            },
        };

        let msg = IotCodec::encode_message(&msg.into()).unwrap();
        self.connection.write(&msg).unwrap();
    }

    pub fn sub_c2d(
        &mut self,
        mode: DeliveryGuarantees,
        msg_handler: Box<C2DHandler>,
        error_handler: Box<SubErrorHandler>,
    ) {
        let device_id = match &self.client_id {
            ClientIdentity::Module(_) => panic!("OMG I'm a MODULE!"),
            ClientIdentity::Device(x) => x,
        };

        let packet_id = self.packets_numerator.next();

        let msg = C2DSub {
            packet_id,
            device_id: device_id.clone(),
            mode,
        };
        let msg = IotCodec::encode_message(&msg.into()).unwrap();
        self.c2d = SubState::Subscribing(msg_handler, error_handler, packet_id);
        self.connection.write(&msg).unwrap();
    }

    pub fn sub_twin_updates(&mut self, mode: DeliveryGuarantees, handler: Box<TwinUpdatesHandler>) {
        let packet_id = self.packets_numerator.next();
        let msg = TwinUpdatesSub { packet_id, mode };
        let msg = IotCodec::encode_message(&msg.into()).unwrap();
        self.twin_updates = SubState::Subscribing(
            handler,
            Box::new(|e| println!("Twin updates sub error: {}", e)),
            packet_id,
        );
        self.connection.write(&msg).unwrap();
    }

    pub fn read_twin(&mut self) {
        match self.twin_read {
            SubState::Subscribed(_) => self.request_twin(),
            SubState::Unsubscribed => self.sub_twin_reads(),
            SubState::Subscribing(_, _, _) => {}
        }
    }

    fn request_twin(&mut self) {
        let read_req = ReadTwinReq {
            request_id: format!("{}", uuid::Uuid::new_v4()),
            packet_id: Some(self.packets_numerator.next()),
        };
        let read_req = IotCodec::encode_message(&read_req.into()).unwrap();
        self.connection.write(&read_req).unwrap();
    }

    fn sub_twin_reads(&mut self) {
        let packet_id = self.packets_numerator.next();
        let msg = TwinReadSub {
            mode: DeliveryGuarantees::AtLeastOnce,
            packet_id,
        };
        let msg = IotCodec::encode_message(&msg.into()).unwrap();
        self.connection.write(&msg).unwrap();
        self.twin_read = SubState::Subscribing(
            Box::new(|twin| println!("Got TWIN! {:?}", &twin)),
            Box::new(|e| println!("Error subbing to twin: {}", e)),
            packet_id,
        );
    }

    pub fn process(&mut self) {
        const MAX_TASK_DURATION: Duration = Duration::from_millis(5);
        self.connection.send_task(MAX_TASK_DURATION).unwrap();
        self.connection.recv_task(MAX_TASK_DURATION).unwrap();
        loop {
            match self.connection.read().unwrap() {
                None => {
                    /* Nothing to read */
                    trace!("Got nothing");
                    break;
                }
                Some(packet) => {
                    debug!("Got packet: {:?}", packet);
                    let msg = IotCodec::decode_packet(packet).unwrap();
                    self.process_msg(msg);
                }
            }
        }
        trace!("Process function completed");
    }

    fn process_msg(&mut self, msg: MsgFromHub) {
        debug!("Processing incoming msg: {:?}", msg);
        match msg {
            MsgFromHub::SubscriptionResponseMessage(res) => {
                self.process_sub_res(res);
            }
            MsgFromHub::CloudToDeviceMessage(c2d) => {
                if let SubState::Subscribed(ref mut handler) = self.c2d {
                    debug!("Processing C2D: {:?}", c2d);
                    handler(c2d);
                } else {
                    debug!("Got C2D but no handler was set");
                }
            }
            MsgFromHub::DirectMethodInvocation(dmi) => {
                if let SubState::Subscribed(ref mut handler) = self.dmi {
                    debug!("Processing DMI: {:?}", dmi);
                    handler(dmi);
                } else {
                    debug!("Got DMI but no handler was set");
                }
            }
            MsgFromHub::DesiredPropertiesUpdated(props) => {
                if let SubState::Subscribed(ref mut handler) = self.twin_updates {
                    debug!("Processing Desired Props Update: {:?}", props);
                    handler(props);
                }
            }
            _ => {}
        }
    }

    fn process_sub_res(&mut self, res: SubRes) {
        if self.twin_read.try_complete(&res) {
            debug!("Subscribed to Twin Reads");
            return
        };

        if self.c2d.try_complete(&res) {
            debug!("Subscribed to C2D");
            return
        };

        if self.dmi.try_complete(&res) {
            debug!("Subscribed to Direct Methods");
            return
        };

        if self.twin_updates.try_complete(&res) {
            debug!("Subscribed to Twin Updates");
            return
        };
    }
}
