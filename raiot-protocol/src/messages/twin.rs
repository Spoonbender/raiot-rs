use crate::qos::{DeliveryGuarantees, PacketId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;


/// The Twin
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg(feature = "twin")]
pub struct Twin {
    /// Desired properties section
    pub desired: HashMap<String, Value>,
    
    /// Reported properties section
    pub reported: HashMap<String, Value>,
}

#[cfg(feature = "twin")]
impl Twin {
    /// New Twin
    pub fn new(twin_json: &str) -> Twin {
        let result: Twin = serde_json::from_str(twin_json).unwrap();
        return result;
    }
}

/// Subscribe to Twin read response messages
#[cfg(feature = "twin")]
#[derive(Copy, Clone, Debug)]
pub struct TwinReadSub {
    /// Packet ID
    pub packet_id: PacketId,

    /// Subscription mode of this registration (QoS)
    /// Determines if the Hub requires an acknowledgement of C2D messages
    pub mode: DeliveryGuarantees,
}

/// A command message requesting the IoT Hub to respond with the content of the Twin
#[cfg(feature = "twin")]
#[derive(Clone, Debug)]
pub struct ReadTwinReq {
    /// Request identifier, returned in the ReadTwinRes response message
    pub request_id: String,

    /// Packet ID
    pub packet_id: Option<PacketId>,
}

/// Twin read response message
#[cfg(feature = "twin")]
#[derive(Clone, Debug)]
pub struct ReadTwinRes {
    /// Packet ID
    pub packet_id: Option<PacketId>,

    /// The request identifier specified in the ReadTwinReq
    pub request_id: String,

    /// Response status code, 
    pub status_code: StatusCode,
    
    /// Twin content
    pub body: Option<Value>,
    
    /// Twin version
    pub version: Option<u64>,
}

/// Subscribe to Twin update notifications
#[cfg(feature = "twin")]
#[derive(Copy, Clone, Debug)]
pub struct TwinUpdatesSub {
    /// Packet ID
    pub packet_id: PacketId,
    
    /// Subscription QoS level
    pub mode: DeliveryGuarantees,
}

/// Event message specifying the twin's Desired Properties section was updated
#[cfg(feature = "twin")]
#[derive(Clone, Debug)]
pub struct DesiredPropsUpdated {
    /// Packet ID
    pub packet_id: Option<PacketId>,

    /// The Desired Properties section of the Twin
    pub body: Map<String, Value>,

    /// The version of the Desired Properties section
    pub desired_properties_version: u64,
}

/// Command message for updating the Reported Properties section of the Twin
#[cfg(feature = "twin")]
#[derive(Clone, Debug)]
pub struct UpdateReportedPropsReq {
    /// Request identifier
    pub request_id: String,

    /// Updated Reported Properties section
    pub reported: Map<String, Value>,

    /// Packet ID
    pub packet_id: Option<PacketId>,
}

/// Response code
#[derive(Copy, Clone, Debug)]
#[cfg(feature = "twin")]
pub enum StatusCode {
    /// Command succeeded and returned content
    OK(),
    
    /// Command succeeded and returned no content
    NoContent(),

    /// Throttled
    TooManyRequests(),
    
    /// Client send a bad request
    BadRequest(),
    
    /// Server returned an error
    ServerError(u16),
    
    /// Server sent some unknown status code
    UnknownStatusCode(u16),
}
