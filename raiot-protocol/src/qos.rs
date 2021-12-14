use fmt::Display;
use std::fmt;

/// Represents a single packet identifier
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct PacketId {
    value: u16,
}

impl PacketId {
    /// The packet id
    pub fn value(&self) -> u16 {
        self.value
    }
}

impl From<PacketId> for u16 {
    fn from(packet_id: PacketId) -> Self {
        packet_id.value
    }
}

impl From<u16> for PacketId {
    fn from(packet_id: u16) -> Self {
        PacketId { value: packet_id }
    }
}

impl Display for PacketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// The subscription's delivery guarantees (QoS level)
#[derive(Debug, Copy, Clone)]
pub enum DeliveryGuarantees {
    /// QoS0 - messages will be delivered without requiring an ACK
    AtMostOnce,
    /// QoS1 - messages will require an ACK. If an ACK is not received, the message will be re-delivered.
    AtLeastOnce,
}

/// Determines if we start a clean session, or resume the previous session (dirty).
/// Applies to subscriptions with delivery guarantees of "at least once":
/// When starting a dirty session, any unacknowledged message from the hub to the device will be retransmitted.
/// When starting a clean session, the hub will discard any unacknowledged message
#[derive(Copy, Clone, Debug)]
pub enum SessionMode {
    /// Start a clean session (discard previously-unacknowledged messages)
    Clean,

    /// Start a dirty session (retransmit any previously-unacknowledged messages)
    Dirty,
}
