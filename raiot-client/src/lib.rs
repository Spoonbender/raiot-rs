extern crate futures;
#[macro_use]
extern crate log;

use raiot_client_base::PacketsNumerator;
use iot_socket::{IotSocket, IotSocketTx, MessageFuture, MsgTxResult};
use raiot_protocol::auth::{DeviceCredentials, sas::SasToken};
use raiot_protocol::*;
use raiot_protocol::messages::c2d::*;
use raiot_protocol::messages::direct_methods::*;
use raiot_protocol::messages::telemetry::*;


use raiot_streams::IoStream;
use raiot_streams::{open_nonblocking_stream, NonblockingSocket, ClientCertificate};
use std::collections::HashMap;
use std::future::*;
use std::io::ErrorKind;
use std::sync::{
    mpsc::{channel, Receiver, Sender, TryRecvError},
    Arc, Mutex,
};
use std::thread;
use std::{
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};

use qos::{DeliveryGuarantees, PacketId, SessionMode};
use uuid::Uuid;
use dmi::{DMIRequest, DMIHandler};
use c2d::{C2DMsg, C2DHandler};
use d2c::D2CMsg;
use direct_methods::DirectMethodsSub;
use twin::*;

pub mod iot_socket;
pub mod dmi;
pub mod c2d;
pub mod d2c;



enum DeviceCommand {
    ReadTwin,
    SendTelemetry(D2CMsg),
}

struct RequestState {
    result: Option<Result<MsgFromHub, ()>>,
    waker: Option<Waker>,
}

pub struct TwinFuture {
    state: Arc<Mutex<RequestState>>,
}

impl Future for TwinFuture {
    type Output = ReadTwinRes;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut shared_state = self.state.lock().unwrap();
        match shared_state.result.take() {
            None => {
                shared_state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
            Some(msg) => match msg.unwrap() {
                MsgFromHub::TwinResponseMessage(resp) => Poll::Ready(resp),
                _ => panic!("Wrong msg"),
            },
        }
    }
}

pub struct DeviceClient {
    tx: IotSocketTx,
    id: ClientIdentity,
    packet_id: PacketsNumerator,
    subscribed_to_twin: bool,
    awaiting_response: Arc<Mutex<HashMap<String, Arc<Mutex<RequestState>>>>>,
    dmi_handler: Arc<Mutex<Option<DMIHandler>>>,
    c2d_handler: Arc<Mutex<Option<C2DHandler>>>,
}



impl DeviceClient {
    pub fn set_c2d_handler(&mut self, handler: C2DHandler, mode: DeliveryGuarantees) {
        let old = self.c2d_handler.lock().unwrap().replace(handler);
        if old.is_none() {
            self.tx.send(C2DSub {
                device_id: match self.id {
                    ClientIdentity::Device(ref device) => device.clone(),
                    ClientIdentity::Module(_) => panic!("Cannot subscribe to C2D messages on a module")
                },
                packet_id: self.packet_id.next(),
                mode,
            });
        }
    }

    pub fn set_dmi_handler(&mut self, handler: DMIHandler, mode: DeliveryGuarantees) {
        let old = self.dmi_handler.lock().unwrap().replace(handler);
        if old.is_none() {
            self.tx.send(DirectMethodsSub {
                packet_id: self.packet_id.next(),
                mode,
            });
        }
    }

    pub fn new(id: ClientIdentity, socket: IotSocket) -> DeviceClient {
        let (tx, mut rx) = socket.split();
        let another_tx = tx.clone();
        let client = DeviceClient {
            tx,
            id,
            packet_id: PacketsNumerator::new(),
            subscribed_to_twin: false,
            awaiting_response: Arc::new(Mutex::new(HashMap::new())),
            dmi_handler: Arc::new(Mutex::new(None)),
            c2d_handler: Arc::new(Mutex::new(None)),
        };

        let awaiting_response2 = client.awaiting_response.clone();
        let dmi_handler = client.dmi_handler.clone();
        let c2d_handler = client.c2d_handler.clone();

        thread::spawn(move || loop {
            let msg = rx.recv();
            // debug!("READ LOOP got: {:?}", msg);
            match msg {
                MsgFromHub::TwinResponseMessage(resp) => {
                    if let Some(x) = awaiting_response2.lock().unwrap().remove(&resp.request_id) {
                        let mut y = x.lock().unwrap();
                        y.result = Some(Ok(resp.into()));
                        if let Some(waker) = y.waker.take() {
                            waker.wake();
                        }
                    }
                }
                MsgFromHub::DirectMethodInvocation(dmi) => {
                    let handler_guard = dmi_handler.lock().unwrap();
                    let mut tx2 = another_tx.clone();
                    if let Some(handler) = *handler_guard {
                        thread::spawn(move || {
                            let dmi_result = handler(DMIRequest {
                                method_name: dmi.method_name,
                                body: dmi.body,
                            });
                            tx2.send(DirectMethodRes {
                                packet_id: None,
                                status: dmi_result.status,
                                request_id: dmi.request_id,
                                payload: dmi_result.payload,
                            })
                        });
                    } else {
                        debug!("Got DMI but no handler!");
                        tx2.send(DirectMethodRes {
                            packet_id: None,
                            status: 501,
                            request_id: dmi.request_id,
                            payload: None,
                        });
                    }
                }
                MsgFromHub::CloudToDeviceMessage(c2d) => {
                    let handler_guard = c2d_handler.lock().unwrap();
                    let mut tx2 = another_tx.clone();
                    if let Some(handler) = *handler_guard {
                        thread::spawn(move || {
                            let c2d_result = handler(C2DMsg {
                                props: c2d.props,
                                body: c2d.body,
                            });
                            if let Some(packet_id) = c2d.packet_id {
                                tx2.send(AckMsg { packet_id });
                            }
                        });
                    } else {
                        debug!("Got C2D msg but no handler!");
                    }
                }
                _ => {}
            }
        });

        client
    }

    pub async fn send_telemetry(&mut self, msg: D2CMsg) -> MsgTxResult {
        let msg = TelemetryMsg {
            client_id: self.id.clone(),
            content: msg.content,
            headers: msg.headers,
            packet_id: Some(self.packet_id.next()),
        };

        self.tx.send(msg).await
    }

    pub async fn read_twin(&mut self) -> ReadTwinRes {
        if !self.subscribed_to_twin {
            let sub_msg = TwinReadSub {
                packet_id: self.packet_id.next(),
                mode: DeliveryGuarantees::AtLeastOnce,
            };

            self.tx.send(sub_msg).await.unwrap();
            self.subscribed_to_twin = true;
            debug!("Subscribed to twin!");
        }

        let request_id = Uuid::new_v4().to_string();
        let read_msg = ReadTwinReq {
            request_id: request_id.clone(),
            packet_id: Some(self.packet_id.next()),
        };

        let fut: TwinFuture;
        {
            let mut col = self.awaiting_response.lock().unwrap();
            let request_state = Arc::new(Mutex::new(RequestState {
                result: None,
                waker: None,
            }));
            fut = TwinFuture {
                state: request_state.clone(),
            };
            col.insert(request_id, request_state);
        }

        self.tx.send(read_msg).await.unwrap();

        fut.await
    }
}