use crate::{qos::PacketId, ClientIdentity, PropertyBag};

/// A device-to-cloud message
#[derive(Clone, Debug)]
#[cfg(feature = "telemetry")]
pub struct TelemetryMsg {
    /// The sender's identity
    pub client_id: ClientIdentity,

    /// The content of the message
    pub content: Option<serde_json::Value>,

    /// Packet ID
    pub packet_id: Option<PacketId>,

    /// Message headers
    pub headers: Option<PropertyBag>,
}
