use connect::{ConnectMsg, ConnectRes};
use futures::Future;
use qos::PacketId;
use raiot_buffers::CircularBuffer;
use raiot_client_base::ConnectionSettings;
use raiot_mqtt::packets::MqttPacketizer;
use raiot_protocol::auth::sas::SasToken;
use raiot_protocol::auth::DeviceCredentials;
use raiot_protocol::*;
use raiot_streams::IoStream;
use raiot_streams::{open_nonblocking_stream, ClientCertificate, NonblockingSocket};
use std::io::ErrorKind;
use std::sync::{
    mpsc::{channel, Receiver, Sender, TryRecvError},
    Arc, Condvar, Mutex,
};
use std::thread;
use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

pub type ConnectionResults = Result<IoStream, ConnectRes>;

pub type MsgTxResult = Result<(), ()>;
enum MsgStatus {
    Pending,
    Sent,
    SendFailed,
    Acknowledged,
    Rejected,
    TimedOut,
}

impl From<SubRes> for MsgStatus {
    fn from(resp: SubRes) -> MsgStatus {
        match resp.result {
            Ok(_) => MsgStatus::Acknowledged,
            Err(_) => MsgStatus::Rejected,
        }
    }
}

struct MessageState {
    status: MsgStatus,
    waker: Option<Waker>,
}

pub struct MessageFuture {
    state: Arc<Mutex<MessageState>>,
    ack_required: bool,
}

impl Future for MessageFuture {
    type Output = MsgTxResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut shared_state = self.state.lock().unwrap();
        match &shared_state.status {
            MsgStatus::Pending => {
                shared_state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
            MsgStatus::SendFailed => Poll::Ready(Err(error!("Send Failed"))),
            MsgStatus::TimedOut => Poll::Ready(Err(error!("Timeout"))),
            MsgStatus::Sent => {
                if self.ack_required {
                    shared_state.waker = Some(cx.waker().clone());
                    Poll::Pending
                } else {
                    Poll::Ready(Ok(()))
                }
            }
            MsgStatus::Acknowledged => Poll::Ready(Ok(())),
            MsgStatus::Rejected => Poll::Ready(Err(error!("Rejected"))),
        }
    }
}

struct MessageInFlight {
    msg: MsgToHub,
    state: Arc<Mutex<MessageState>>,
}

pub struct IotSocket {
    outgoing: IotSocketTx,
    incoming: IotSocketRx,
}

#[derive(Debug, Clone)]
pub struct IotSocketTx {
    outgoing: Sender<MessageInFlight>,
}

pub struct IotSocketRx {
    incoming: Receiver<MsgFromHub>,
}

impl IotSocketTx {
    pub fn send<M: Into<MsgToHub>>(&mut self, msg: M) -> MessageFuture {
        let state = MessageState {
            waker: None,
            status: MsgStatus::Pending,
        };

        let state = Arc::new(Mutex::new(state));

        let msg = msg.into();
        let ack_required = msg.packet_id().is_some();
        self.outgoing
            .send(MessageInFlight {
                msg,
                state: state.clone(),
            })
            .unwrap();

        MessageFuture {
            state,
            ack_required,
        }
    }
}

impl IotSocketRx {
    pub fn try_recv(&mut self) -> Option<MsgFromHub> {
        match self.incoming.try_recv() {
            Ok(msg) => Some(msg),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                panic!("OMG OMG OMG I'm disco'd from the origin of incoming msgs")
            }
        }
    }

    pub fn recv(&mut self) -> MsgFromHub {
        match self.incoming.recv() {
            Ok(msg) => msg,
            Err(_) => panic!("Hung up!"),
        }
    }
}
impl IotSocket {
    pub fn split(self) -> (IotSocketTx, IotSocketRx) {
        (self.outgoing, self.incoming)
    }

    pub fn connect(settings: ConnectionSettings) -> IotSocket {
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();
        let socket = IotSocket {
            outgoing: IotSocketTx { outgoing: tx1 },
            incoming: IotSocketRx { incoming: rx2 },
        };

        let settings = settings.clone();

        let pair = Arc::new((Mutex::new(false), Condvar::new()));
        let pair2 = pair.clone();

        thread::spawn(move || {
            let connection_result = connect(&settings);

            let stream = match connection_result {
                Ok(stream) => stream,
                Err(e) => panic!("OMG this just happened! {}", e),
            };

            {
                let (lock, cvar) = &*pair2;
                let mut connected = lock.lock().unwrap();
                *connected = true;
                cvar.notify_one();
            }

            let mut ctl = IotSocketCtl {
                incoming_queue: tx2,
                outgoing_queue: rx1,
                settings,
                stream,
                awaiting_acks: HashMap::new(),
                total_bytes_read: 0,
                total_bytes_written: 0,
                tx_buf: None,
                encoding_buf: vec![1u8; 256 * 1024].into_boxed_slice(),
                packetizer: MqttPacketizer::new(),
                write_buffer: CircularBuffer::new(256 * 1024),
            };
            ctl.socket_loop();
        });

        let (lock, cvar) = &*pair;
        let mut started = lock.lock().unwrap();
        while !*started {
            started = cvar.wait(started).unwrap();
        }

        socket
    }

    pub fn send<M: Into<MsgToHub>>(&mut self, msg: M) -> MessageFuture {
        self.outgoing.send(msg)
    }

    pub fn try_recv(&mut self) -> Option<MsgFromHub> {
        self.incoming.try_recv()
    }
}

struct IotSocketCtl {
    settings: ConnectionSettings,
    outgoing_queue: Receiver<MessageInFlight>,
    incoming_queue: Sender<MsgFromHub>,
    stream: IoStream,
    awaiting_acks: HashMap<PacketId, Arc<Mutex<MessageState>>>,
    total_bytes_read: u64,
    total_bytes_written: u64,
    packetizer: MqttPacketizer,
    write_buffer: CircularBuffer,
    encoding_buf: Box<[u8]>,
    tx_buf: Option<MessageInFlight>,
}

impl IotSocketCtl {
    pub fn total_bytes_written(&self) -> u64 {
        self.total_bytes_written
    }

    pub fn total_bytes_read(&self) -> u64 {
        self.total_bytes_read
    }

    pub fn recv_next(&mut self) -> bool {
        loop {
            if let Some(packet) = self.packetizer.get_next_packet().unwrap() {
                match IotCodec::decode_packet(packet) {
                    Ok(msg) => {
                        self.handle_incoming_msg(msg);
                        return true;
                    }
                    Err(e) => {
                        warn!("Failure decoding message from server: {}", e);
                        panic!("Failure decoding IoT packet");
                    }
                }
            } else {
                // we don't have a complete packet, keep reading from the buffer
                match self.packetizer.append_from_reader(&mut self.stream) {
                    // Nothing to read from the socket, go do other things
                    Ok(0) => return false,
                    // Got something from the buffer, keep iterating - we might have a complete packet
                    Ok(amount) => self.total_bytes_read += amount as u64,
                    Err(e) if e.kind() == ErrorKind::WouldBlock => return false,
                    Err(e) if e.kind() == ErrorKind::Interrupted => return true,
                    Err(e) => panic!("OMG could NOT read! {:?}", e),
                }
            }
        }
    }

    pub fn send_next(&mut self) -> bool {
        if let Some(msg) = self.take_next_outgoing_msg() {
            // we have an outgoing message at hand, let's try and send it
            debug!("Sending a message");

            let encoded_length = IotCodec::encode(&msg.msg, &mut self.encoding_buf)
                .expect("Encoding must work, though in fact it didn't");

            if let Some(packet_id) = msg.msg.packet_id() {
                if !self.awaiting_acks.contains_key(&packet_id) {
                    self.awaiting_acks.insert(packet_id, msg.state.clone());
                }
            }

            let send_result = self.stream.try_send(&self.encoding_buf[0..encoded_length]);

            match send_result {
                Ok(()) => {
                    debug!("Message sent");
                    let mut state = msg.state.lock().unwrap();
                    self.total_bytes_written += encoded_length as u64;
                    state.status = MsgStatus::Sent;
                    return true;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    self.tx_buf = Some(msg);
                    return false;
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {
                    self.tx_buf = Some(msg);
                    return true;
                }
                Err(e) => {
                    debug!("Send failed: {:?}", e);
                    let mut state = msg.state.lock().unwrap();
                    state.status = MsgStatus::SendFailed;
                    return true;
                }
            }
        } else {
            return false;
        }
    }

    fn take_next_outgoing_msg(&mut self) -> Option<MessageInFlight> {
        if let None = self.tx_buf {
            self.tx_buf = match self.outgoing_queue.try_recv() {
                Ok(msg) => Some(msg),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    panic!("OMG OMG OMG I'm disco'd from the origin of TX")
                }
            };
        }

        return self.tx_buf.take();
    }

    fn socket_loop(&mut self) {
        debug!("Starting loop");
        loop {
            // Transmit pending TX messages
            while self.send_next() {}

            // Get pending RX messages
            while self.recv_next() {}

            thread::sleep(Duration::from_millis(1));
        }
    }

    fn handle_incoming_msg(&mut self, msg: MsgFromHub) {
        match msg {
            MsgFromHub::SubscriptionResponseMessage(resp) => {
                self.handle_ack(resp.packet_id, resp.into());
            }
            MsgFromHub::PublicationSucceeded(packet_id) => {
                self.handle_ack(packet_id, MsgStatus::Acknowledged);
            }
            other => {
                self.incoming_queue.send(other).unwrap();
            }
        }
    }

    fn handle_ack(&mut self, packet_id: PacketId, result: MsgStatus) {
        if let Some(item) = &self.awaiting_acks.remove(&packet_id) {
            let mut state = item.lock().unwrap();
            state.status = result;
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        }
    }
}

fn generate_sas_token(settings: &ConnectionSettings, key: &str) -> SasToken {
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

fn connect(settings: &ConnectionSettings) -> ConnectionResults {
    let now = Instant::now();
    let client_certificate = match settings.credentials {
        DeviceCredentials::Certificate(ref cert) => Some(ClientCertificate {
            bytes: cert.bytes.clone(),
            password: cert.password.clone(),
        }),
        DeviceCredentials::Sas(_) => None,
    };

    let mut stream = open_nonblocking_stream(
        &settings.hostname,
        settings.port.into(),
        settings.timeout,
        client_certificate.as_ref(),
    )
    .unwrap();

    let token = match settings.credentials {
        DeviceCredentials::Sas(ref key) => Some(generate_sas_token(settings, key).into()),
        DeviceCredentials::Certificate(_) => None,
    };

    let conn = ConnectMsg {
        client_id: settings.client_id.clone(),
        server_addr: settings.hostname.clone(),
        sas_token: token,
        session_mode: settings.session_mode,
    };

    let mut buf = vec![0u8; 1024 * 1024];
    debug!("Connecting MQTT...");

    let encoded_size = IotCodec::encode(&conn.into(), &mut buf).unwrap();
    debug!("Sending CONN...");
    &mut stream.send(&buf[0..encoded_size]).unwrap();
    debug!("Waiting...");

    loop {
        if now.elapsed() >= settings.timeout {
            return Err(ConnectRes::Timeout);
        }
        let read_res = &mut stream.try_read();
        match read_res {
            Ok(Some(bytes)) => {
                return decode_connect_response(&bytes, stream);
            }
            Ok(None) => {
                debug!("Nothing to read");
                // TODO is EOL ??
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                debug!("Would Block");
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                debug!("Interrupted");
                thread::sleep(Duration::from_millis(1)); // TODO retry immediately?
            }
            Err(e) => {
                debug!("Some other IO error");
                return Err(ConnectRes::IOError(e.kind()));
            }
        }
    }
}

fn decode_connect_response(bytes: &Vec<u8>, stream: IoStream) -> ConnectionResults {
    debug!("decode_connect_response, bytes length: {}", bytes.len());
    let mut packetizer = MqttPacketizer::new();
    packetizer.append_all_bytes(&bytes[0..bytes.len()]).unwrap();
    match IotCodec::decode_packet(packetizer.get_next_packet().unwrap().unwrap()) {
        Ok(MsgFromHub::ConnectResponseMessage(ConnectRes::Accepted)) => Ok(stream),
        Ok(MsgFromHub::ConnectResponseMessage(error)) => Err(error),
        Ok(_other) => {
            debug!("Unexpected message type");
            Err(ConnectRes::ProtocolViolation)
        }
        Err(_e) => {
            debug!("Failure decoding response");
            Err(ConnectRes::ProtocolViolation)
        }
    }
}
