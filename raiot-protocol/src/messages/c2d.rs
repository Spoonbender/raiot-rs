use crate::{qos::DeliveryGuarantees, qos::PacketId, DeviceIdentity, PropertyBag};
use std::fmt::{self, Formatter};

/// Represents a request to subscribe to C2D messages
#[cfg(feature = "c2d")]
#[derive(Clone, Debug)]
pub struct C2DSub {
    /// Identifies of this packet, which will appear in the matching Acknowledgement message
    pub packet_id: PacketId,
    /// The subscribing device's identity
    pub device_id: DeviceIdentity,

    /// Subscription mode of this registration (QoS)
    /// Determines if the Hub requires an acknowledgement of C2D messages
    pub mode: DeliveryGuarantees,
}

/// Represents a single C2D message
#[cfg(feature = "c2d")]
#[derive(Clone, Debug)]
pub struct C2DMsg {
    /// Packet Identifier
    /// Only present if QoS1 is used
    pub packet_id: Option<PacketId>,

    /// The message body (if any)
    pub body: Option<String>,

    /// The recipient device ID
    pub device_id: String,

    /// Message properties, if any
    pub props: Option<PropertyBag>,
}

impl fmt::Display for C2DMsg {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Props: {:?}, Body: {:?}, PacketID: {:?}",
            self.body, self.props, self.packet_id
        )
    }
}
