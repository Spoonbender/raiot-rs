use std::io::ErrorKind;
use std::{
    io::{Read, Write},
    time::Duration,
    time::Instant,
};

use crate::packets::{MqttPacketizer, MqttStreamer};
use log::{debug, trace};
use mqtt::packet::*;
use mqtt::{control::variable_header::ConnectReturnCode, packet::ConnackPacket};

pub enum MqttConnectError<S: Read + Write> {
    WouldBlock(MqttConnectionInProgress<S>),
    ConnectFailed(ConnectReturnCode),
    IOError(ErrorKind),
    ProtocolViolation,
}

pub struct MqttConnector<S: Read + Write> {
    stream: S,
    tx_buffer_size: usize,
    rx_buffer_size: usize,
    connect_timeout: Duration,
}

pub struct MqttConnection<S: Read + Write> {
    packetizer: MqttPacketizer,
    streamer: MqttStreamer,
    stream: S,
}

impl<S: Read + Write> MqttConnection<S> {
    /// Writes a packet to the tx buffer.
    pub fn write(&mut self, packet: &VariablePacket) -> std::io::Result<()> {
        debug!("Writing a packet");
        self.streamer.write_packet(packet)
    }

    /// Reads the next packet from the rx buffer, if any.
    pub fn read(&mut self) -> std::io::Result<Option<VariablePacket>> {
        if let Some(packet) = self.packetizer.get_next_packet()? {
            Ok(Some(packet))
        } else {
            Ok(None)
        }
    }

    /// Sends bytes from the tx buffer until blocked or until the alloted time is exhausted
    /// Returns the amount of data still pending in the buffer
    pub fn send_task(&mut self, timeout: Duration) -> std::io::Result<usize> {
        trace!("send_task starting");
        let start = Instant::now();
        loop {
            if start.elapsed() >= timeout {
                trace!("Write timed out");
                return Ok(self.streamer.data_size());
            }

            if self.streamer.is_empty() {
                trace!("TX buffer empty");
                return Ok(0);
            }

            match self.streamer.write_into(&mut self.stream) {
                Ok(size) => {
                    debug!("Wrote from TX buffer to socket: {}", size);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {
                    trace!("Write interrupted");
                    // keep trying!
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    trace!("Cannot write to socket: would block");
                    return Ok(self.streamer.data_size());
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    /// Tries to read data from the socket until a complete packet is buffered, or until blocked, or the alloted time is exhausted.
    pub fn recv_task(&mut self, timeout: Duration) -> std::io::Result<Option<VariablePacket>> {
        trace!("recv_task starting");
        let start = Instant::now();
        loop {
            if start.elapsed() >= timeout {
                debug!("read timed out");
                return Ok(None);
            }

            match self.packetizer.append_from_reader(&mut self.stream) {
                Ok(_size) => {
                    // Perhaps we go a full packet now?
                    debug!("read: {:?}", _size);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {
                    // keep trying!
                    debug!("read interrupted");
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    trace!("read would block");
                    return Ok(None);
                }
                Err(e) => {
                    debug!("read failed");
                    return Err(e);
                }
            }
        }
    }
}

pub struct MqttConnectionInProgress<S: Read + Write> {
    packetizer: MqttPacketizer,
    streamer: MqttStreamer,
    stream: S,
    stopwatch: Instant,
    connect_timeout: Duration,
}

impl<S: Read + Write> MqttConnector<S> {
    pub fn create(stream: S) -> MqttConnector<S> {
        MqttConnector {
            stream,
            tx_buffer_size: 512 * 1024,
            rx_buffer_size: 512 * 1024,
            connect_timeout: Duration::from_secs(10),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn with_rx_buffer(mut self, size: usize) -> Self {
        self.rx_buffer_size = size;
        self
    }

    pub fn with_tx_buffer(mut self, size: usize) -> Self {
        self.tx_buffer_size = size;
        self
    }

    pub fn connect(
        self,
        connect_packet: ConnectPacket,
    ) -> std::io::Result<MqttConnectionInProgress<S>> {
        let packetizer = MqttPacketizer::with_buffer_size(self.rx_buffer_size);
        let mut streamer = MqttStreamer::with_buffer_size(self.tx_buffer_size);
        streamer.write_packet(&connect_packet.into())?;
        let stream = self.stream;
        let conn = MqttConnectionInProgress {
            packetizer,
            streamer,
            stream,
            connect_timeout: self.connect_timeout,
            stopwatch: Instant::now(),
        };
        Ok(conn)
    }
}

impl<S: Read + Write> MqttConnectionInProgress<S> {
    pub fn complete(mut self) -> Result<MqttConnection<S>, MqttConnectError<S>> {
        if self.stopwatch.elapsed() > self.connect_timeout {
            return Err(MqttConnectError::IOError(ErrorKind::TimedOut.into()));
        }

        if !self.streamer.is_empty() {
            match self.send_next() {
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    return Err(MqttConnectError::WouldBlock(self))
                }
                Ok(()) => {
                    // Done sending the CONNECT packet, now we need to wait for CONNACK
                }
                Err(e) => return Err(MqttConnectError::IOError(e.kind())),
            }
        }

        loop {
            match self.packetizer.append_from_reader(&mut self.stream) {
                Ok(0) => return Err(MqttConnectError::IOError(ErrorKind::ConnectionAborted)),
                Ok(_size) => {}
                Err(e) if e.kind() == ErrorKind::Interrupted => {
                    // keep looping, hoping we won't get interrupted endlessly...
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(MqttConnectError::IOError(e.kind())),
            }
        }

        match self.packetizer.get_next_packet() {
            Ok(None) => {
                return Err(MqttConnectError::WouldBlock(self));
            }
            Ok(Some(VariablePacket::ConnackPacket(packet))) => {
                return self.process_connack(packet);
            }
            Ok(Some(_other_packet)) => {
                // Any non-CONNACK response is a protocol violation
                return Err(MqttConnectError::ProtocolViolation);
            }
            Err(e) if e.kind() == ErrorKind::InvalidData => {
                return Err(MqttConnectError::ProtocolViolation);
            }
            Err(_e) => {
                panic!("Some unexpected error!");
            }
        }
    }

    fn process_connack(
        self,
        packet: ConnackPacket,
    ) -> Result<MqttConnection<S>, MqttConnectError<S>> {
        match packet.connect_return_code() {
            ConnectReturnCode::ConnectionAccepted => Ok(MqttConnection {
                packetizer: self.packetizer,
                streamer: self.streamer,
                stream: self.stream,
            }),
            other => Err(MqttConnectError::ConnectFailed(other)),
        }
    }

    fn send_next(&mut self) -> std::io::Result<()> {
        loop {
            let stream = &mut self.stream;
            match self.streamer.write_into(stream) {
                Ok(_written_size) => {
                    if self.streamer.is_empty() {
                        return Ok(());
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    return Err(ErrorKind::WouldBlock.into());
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mqtt::{packet::PublishPacket, Encodable, TopicName};
    use raiot_test_utils::{MockClientSocket, MockServerSocket, MockSocket};

    trait PacketWriter {
        fn push_packet(&mut self, packet: &VariablePacket);
    }

    impl PacketWriter for MockServerSocket {
        fn push_packet(&mut self, packet: &VariablePacket) {
            let mut write_buf: Vec<u8> = Vec::new();
            packet.encode(&mut write_buf).unwrap();
            self.push_data(&write_buf);
        }
    }

    #[test]
    fn test_connection_flow_sanity() {
        let connpack = ConnectPacket::new("clientid");
        let (client_socket, mut server_socket) = MockSocket::create();
        let connack = ConnackPacket::new(false, ConnectReturnCode::ConnectionAccepted);
        server_socket.push_packet(&connack.into());
        server_socket.push_write_ctl(Ok(8 * 1024));
        server_socket.push_read_ctl(Err(ErrorKind::WouldBlock.into()));
        server_socket.push_read_ctl(Ok(8 * 1024));
        let sut = MqttConnector::create(client_socket)
            .connect(connpack)
            .unwrap();

        let res = run_to_completion(sut);
        assert!(res.is_ok());
    }

    #[test]
    fn test_connection_flow_protocol_violation() {
        // Arrange
        let connpack = ConnectPacket::new("clientid");
        let (client_socket, mut server_socket) = MockSocket::create();
        let pubpack = PublishPacket::new(
            TopicName::new("mytopic").unwrap(),
            QoSWithPacketIdentifier::Level0,
            "",
        );
        server_socket.push_packet(&pubpack.into());
        server_socket.push_write_ctl(Ok(8 * 1024));
        server_socket.push_read_ctl(Err(ErrorKind::WouldBlock.into()));
        server_socket.push_read_ctl(Ok(8 * 1024));
        let sut = MqttConnector::create(client_socket)
            .connect(connpack)
            .unwrap();

        // Act
        let res = run_to_completion(sut);

        // Assert
        assert!(res.is_err());
        match res.err().unwrap() {
            MqttConnectError::ProtocolViolation => {}
            _other => assert!(false),
        }
    }

    #[test]
    fn test_connection_flow_tiny_partial_reads_and_writes() {
        // Arrange
        let connpack = ConnectPacket::new("clientid");
        let (client_socket, mut server_socket) = MockSocket::create();
        let connack = ConnackPacket::new(false, ConnectReturnCode::ConnectionAccepted);
        server_socket.push_packet(&connack.into());
        for _ in 1..1000 {
            server_socket.push_write_ctl(Err(ErrorKind::WouldBlock.into()));
            server_socket.push_write_ctl(Ok(1));
        }
        for _ in 1..1000 {
            server_socket.push_read_ctl(Err(ErrorKind::WouldBlock.into()));
            server_socket.push_read_ctl(Ok(1));
        }

        let sut = MqttConnector::create(client_socket)
            .connect(connpack)
            .unwrap();

        // Act
        let res = run_to_completion(sut);

        // Assert
        assert!(res.is_ok());
    }

    #[test]
    fn test_connection_flow_auth_failed() {
        // Arrange
        let connpack = ConnectPacket::new("clientid");
        let (client_socket, mut server_socket) = MockSocket::create();
        let connack = ConnackPacket::new(false, ConnectReturnCode::NotAuthorized);
        server_socket.push_packet(&connack.into());
        server_socket.push_write_ctl(Ok(8 * 1024));
        server_socket.push_read_ctl(Err(ErrorKind::WouldBlock.into()));
        server_socket.push_read_ctl(Ok(8 * 1024));
        let sut = MqttConnector::create(client_socket)
            .connect(connpack)
            .unwrap();

        // Act
        let res = run_to_completion(sut);

        // Assert
        assert!(res.is_err());
        let err: MqttConnectError<MockClientSocket> = res.err().unwrap();
        match err {
            MqttConnectError::ConnectFailed(ConnectReturnCode::NotAuthorized) => {}
            _other => assert!(false),
        }
    }

    #[test]
    fn test_connection_flow_connection_closed() {
        // Arrange
        let connpack = ConnectPacket::new("clientid");
        let (client_socket, mut server_socket) = MockSocket::create();
        server_socket.push_write_ctl(Err(ErrorKind::ConnectionAborted.into()));
        server_socket.push_read_ctl(Ok(8 * 1024));
        let sut = MqttConnector::create(client_socket)
            .connect(connpack)
            .unwrap();

        // Act
        let res = run_to_completion(sut);

        // Assert
        assert!(res.is_err());
        let err: MqttConnectError<MockClientSocket> = res.err().unwrap();
        match err {
            MqttConnectError::IOError(ErrorKind::ConnectionAborted) => {}
            _other => assert!(false),
        }
    }

    #[test]
    fn test_connection_flow_timeout_on_send() {
        // Arrange
        let connpack = ConnectPacket::new("clientid");
        let (client_socket, mut server_socket) = MockSocket::create();
        for _ in 1..1000 {
            server_socket.push_write_ctl(Err(ErrorKind::WouldBlock.into()));
        }
        let sut = MqttConnector::create(client_socket)
            .with_timeout(Duration::from_millis(500))
            .connect(connpack)
            .unwrap();

        // Act
        let res = run_to_completion_with_backoffs(sut);

        // Assert
        assert!(res.is_err());
        let err: MqttConnectError<MockClientSocket> = res.err().unwrap();
        match err {
            MqttConnectError::IOError(ErrorKind::TimedOut) => {}
            _other => assert!(false),
        }
    }

    #[test]
    fn test_connection_flow_sending_oversized_packet() {
        // Arrange
        let connpack = ConnectPacket::new("clientid");
        let (client_socket, mut server_socket) = MockSocket::create();
        server_socket.push_write_ctl(Err(ErrorKind::ConnectionAborted.into()));
        server_socket.push_read_ctl(Ok(8 * 1024));

        // Act
        let res = MqttConnector::create(client_socket)
            .with_tx_buffer(5)
            .connect(connpack);

        // Assert
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    fn run_to_completion(
        mut sut: MqttConnectionInProgress<MockClientSocket>,
    ) -> Result<MqttConnection<MockClientSocket>, MqttConnectError<MockClientSocket>> {
        loop {
            match sut.complete() {
                Ok(conn) => return Ok(conn),
                Err(MqttConnectError::WouldBlock(p)) => {
                    // continue trying
                    sut = p;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn run_to_completion_with_backoffs(
        mut sut: MqttConnectionInProgress<MockClientSocket>,
    ) -> Result<MqttConnection<MockClientSocket>, MqttConnectError<MockClientSocket>> {
        loop {
            match sut.complete() {
                Ok(conn) => return Ok(conn),
                Err(MqttConnectError::WouldBlock(p)) => {
                    // continue trying
                    std::thread::sleep(Duration::from_millis(100));
                    sut = p;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
