pub use crate::packets::packetizer::MqttPacketizer;
use raiot_buffers::CircularBuffer;

use mqtt::packet::VariablePacket;
use mqtt::Encodable;
use std::io::{ErrorKind, Read, Write};

/// Streams MQTT packets into the underlying buffer
pub struct MqttStreamer {
    buffer: CircularBuffer,
}

impl MqttStreamer {
    pub fn with_buffer_size(size: usize) -> MqttStreamer {
        let buffer = CircularBuffer::new(size);

        MqttStreamer { buffer }
    }

    /// Attempts to write a packet into the underlying buffer
    ///
    /// # Errors
    /// - Returns WriteZero if there currently isn't enough free space in the underlying buffer
    /// - Returns InvalidInput if the packet is bigger than the buffer size (and can never be written)
    pub fn write_packet(&mut self, packet: &VariablePacket) -> std::io::Result<()> {
        let length = packet.encoded_length() as usize;
        if length > self.buffer.size() {
            return Err(ErrorKind::InvalidInput.into());
        }
        else if length > self.buffer.available_space() {
            return Err(ErrorKind::WriteZero.into());
        }

        packet.encode(&mut self.buffer)
              .map_err(|_e| ErrorKind::InvalidInput.into())
    }

    /// TRUE if the underlying buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn data_size(&self) -> usize {
        self.buffer.valid_length()
    }

    pub fn write_into<S: Read + Write>(&mut self, writer: &mut S) -> std::io::Result<usize> {
        self.buffer.write_into(writer)
    }
}
