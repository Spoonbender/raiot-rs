use std::fmt::{self, Formatter};

use crate::qos::{DeliveryGuarantees, PacketId};

/// A subscription request to receive direct method invocation requests
#[cfg(feature = "direct-methods")]
#[derive(Copy, Clone, Debug)]
pub struct DirectMethodsSub {
    /// Subscription packet ID
    pub packet_id: PacketId,

    /// The subscription mode
    pub mode: DeliveryGuarantees,
}

/// A request from the IoT Hub to invoke a specific method on the device
#[cfg(feature = "direct-methods")]
#[derive(Clone, Debug)]
pub struct DirectMethodReq {
    /// Packet identifier
    pub packet_id: Option<PacketId>,

    /// Invocation request ID
    pub request_id: String,

    /// The name of the method to invoke
    pub method_name: String,

    /// An optional body of the invocation request
    pub body: Option<serde_json::Value>,
}

impl fmt::Display for DirectMethodReq {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Method: {:?}, Body: {:?}, Request ID: {:?} PacketID: {:?}",
            self.method_name, self.body, self.request_id, self.packet_id
        )
    }
}

/// Represents the result of a direct method invocation request
#[cfg(feature = "direct-methods")]
#[derive(Clone, Debug)]
pub struct DirectMethodRes {
    /// The request ID, as specified in the incoming DirectMethodInvocation message
    pub request_id: String,

    /// The status code of the invocation result
    pub status: i32,

    /// Optional payload returned by the device
    pub payload: Option<serde_json::Value>,

    /// Packet identifier
    pub packet_id: Option<PacketId>,
}
