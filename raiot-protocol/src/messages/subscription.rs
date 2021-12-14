use enum_display_derive::Display;

use std::fmt::Display;

use crate::qos::PacketId;

/// The response to a subscription attempt
#[derive(Copy, Clone, Debug)]
pub struct SubRes {
    /// The ID of the subscription packet
    pub packet_id: PacketId,

    /// The result of the subscription attempt
    pub result: Result<(), SubError>,
}

/// Subscription error
#[derive(Copy, Debug, Clone, Display)]
pub enum SubError {
    /// Timed-out waiting for SUBACK
    Timeout,

    /// Server indicated failure
    Failure
}