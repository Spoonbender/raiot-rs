use mqtt::packet::VariablePacket;

use crate::connection::MqttConnection;
use std::io::{Read, Write};

pub struct MqttSession<S: Read + Write> {
    connection: MqttConnection<S>,
}

impl<S: Read + Write> MqttSession<S> {
    pub fn write(&mut self, packet: &VariablePacket) -> std::io::Result<()> {}

    pub fn read(&mut self) -> std::io::Result<Option<VariablePacket>> {}
}

struct InFlightPackets {
    packets: std::collections::vec_deque::VecDeque<VariablePacket>,
}

impl InFlightPackets {
    fn add(&mut self, packet: VariablePacket) {
        self.packets.push_back()
    }
}
