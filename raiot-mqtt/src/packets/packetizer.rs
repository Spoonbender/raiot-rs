use log::warn;
use mqtt::control::{fixed_header::FixedHeaderError, FixedHeader};
use mqtt::packet::*;
use mqtt::Decodable;
use mqtt::Encodable;
use raiot_buffers::CircularBuffer;
use std::io::{ErrorKind, Read, Write};

/// Turns byte streams into MQTT packets
///
/// The underlying MQTT library used by raiot does not support nonblocking sockets, and would fail if encountered with a WouldBlock error.
/// The MqttPacketizer helps work around this limitation, by attempting to decode an MQTT packet only once the entire packet has been loaded into the buffer.
#[derive(Debug)]
pub struct MqttPacketizer {
    buffer: CircularBuffer,
}

impl MqttPacketizer {
    const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024;
    const MIN_BUFFER_SIZE: usize = 5;

    /// Creates a packetizer with the default buffer size
    pub fn new() -> MqttPacketizer {
        return MqttPacketizer::with_buffer_size(MqttPacketizer::DEFAULT_BUFFER_SIZE);
    }

    /// Creates a packetizer with the specified buffer size
    ///
    /// # Panics
    /// Panics if the specified buffer size is smaller than the minimum allowed buffer size
    pub fn with_buffer_size(size: usize) -> MqttPacketizer {
        assert!(
            size >= MqttPacketizer::MIN_BUFFER_SIZE,
            "MQTT Packetizer buffer must be greater than {} bytes",
            MqttPacketizer::MIN_BUFFER_SIZE
        );

        MqttPacketizer {
            buffer: CircularBuffer::new(size),
        }
    }

    /// Returns the amount of free space in the buffer
    pub fn available_space(&self) -> usize {
        self.buffer.available_space()
    }

    /// Appends bytes to the buffer, accounting for the available space in the buffer
    pub fn append_bytes(&mut self, bytes: &[u8]) -> Result<usize, std::io::Error> {
        let write_size = std::cmp::min(self.available_space(), bytes.len());
        let slice = &bytes[0..write_size];
        self.buffer.append_all_bytes(slice)?;
        Ok(write_size)
    }

    /// Attempts to append all provided bytes to the buffer
    ///
    /// # Errors
    /// Returns WriteZero if there isn't enough available space to write all the provided bytes
    pub fn append_all_bytes(&mut self, bytes: &[u8]) -> Result<(), std::io::Error> {
        self.buffer.append_all_bytes(bytes)
    }

    /// Reads data from the reader into the buffer, until the reader is exhausted or the buffer is full
    pub fn append_from_reader<R: Read>(&mut self, reader: &mut R) -> Result<usize, std::io::Error> {
        self.buffer.append_from_reader(reader)
    }

    /// Attempts to decode the next MQTT packet from the buffer
    ///
    /// # Errors
    /// - Returns InvalidData if the decoded MQTT packet is invalid, or in case the packet being assembled is bigger than the total capacity of the buffer, which means we'll never be able to decode it.
    pub fn get_next_packet(&mut self) -> Result<Option<VariablePacket>, std::io::Error> {
        if self.buffer.valid_length() <= 1 {
            // not enough bytes for a fixed header (minimum is 2), wait for more bytes
            return Ok(None);
        }

        // The MQTT fixed header is actually not "fixed size": its size is between 2 and 5 bytes
        let max_fixed_header_bytes = std::cmp::min(self.buffer.valid_length(), 5);
        let mut fixed_header_bytes = self.buffer.peek(max_fixed_header_bytes);
        match FixedHeader::decode(&mut fixed_header_bytes) {
            Ok(fixed_header) => {
                let fixed_header_length = fixed_header.encoded_length();
                let packet_length = (fixed_header_length + fixed_header.remaining_length) as usize;
                if self.buffer.valid_length() < packet_length {
                    // not all packet bytes arrived
                    if self.buffer.size() < packet_length {
                        // the packet is bigger than the buffer size, and will never fit in.
                        warn!(
                            "Packet size exeeds buffer size. Packet size: {}, buffer size: {}",
                            packet_length,
                            self.buffer.size()
                        );
                        return Err(ErrorKind::InvalidData.into());
                    }
                    // wait for more bytes to arrive...
                    return Ok(None);
                }

                let mut packet_bytes = self.buffer.read_bytes(packet_length);
                let packet = match VariablePacket::decode(&mut packet_bytes) {
                    Ok(packet) => packet,
                    Err(e) => match e {
                        VariablePacketError::IoError(ioe) => return Err(ioe),
                        _other => return Err(ErrorKind::InvalidData.into()),
                    },
                };
                Ok(Some(packet))
            }
            Err(FixedHeaderError::IoError(ioe)) if ioe.kind() == ErrorKind::UnexpectedEof => {
                // Fixed header is incomplete, wait for more bytes, meanwhile we don't have a packet to return...
                Ok(None)
            }
            Err(_e) => Err(ErrorKind::InvalidData.into()),
        }
    }
}

impl Write for MqttPacketizer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.append_bytes(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mqtt::TopicName;

    #[test]
    fn test_packetizer_packet_too_large() {
        let mut sut = MqttPacketizer::with_buffer_size(20);
        let payload = vec![5u8; 1024];
        let packet = PublishPacket::new(
            TopicName::new("mytopic").unwrap(),
            QoSWithPacketIdentifier::Level0,
            payload,
        );

        let mut packet_bytes = Vec::new();
        packet.encode(&mut packet_bytes).unwrap();
        let write_size = sut.write(&packet_bytes).unwrap();
        assert_eq!(write_size, 20);
        let result = sut.get_next_packet();
        assert!(result.is_err());
        assert!(result.unwrap_err().kind() == std::io::ErrorKind::InvalidData);
    }

    fn test_packetizer_partial_packet_test(first_write_size: usize) {
        let mut sut = MqttPacketizer::with_buffer_size(1024);
        let payload = vec![5u8; 900];
        let packet = PublishPacket::new(
            TopicName::new("mytopic").unwrap(),
            QoSWithPacketIdentifier::Level0,
            payload,
        );

        let mut packet_bytes = Vec::new();
        packet.encode(&mut packet_bytes).unwrap();
        let write_size = sut.write(&packet_bytes[0..first_write_size]).unwrap();
        assert_eq!(write_size, first_write_size);
        let result = sut.get_next_packet();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let write_size = sut.write(&packet_bytes[first_write_size..]).unwrap();
        assert_eq!(write_size, packet_bytes.len() - first_write_size);
        let result = sut.get_next_packet();
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_packetizer_packet_is_partial() {
        test_packetizer_partial_packet_test(10);
    }

    #[test]
    fn test_packetizer_partial_fixed_header() {
        test_packetizer_partial_packet_test(2);
    }

    #[test]
    fn test_packetizer_partial_fixed_header_single_byte() {
        test_packetizer_partial_packet_test(1);
    }
}
