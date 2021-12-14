/// Subscription messages
pub mod subscription;

/// Connection flow messages
pub mod connect;

/// Device-to-cloud telemetry messages
#[cfg(feature = "telemetry")]
pub mod telemetry;

/// Cloud-to-device messages
#[cfg(feature = "c2d")]
pub mod c2d;

/// Direct method invocation messages
#[cfg(feature = "direct-methods")]
pub mod direct_methods;

/// Twin-related messages and data structures
#[cfg(feature = "twin")]
pub mod twin;

use connect::{ConnectMsg, ConnectRes};

use crate::qos::PacketId;
use std::{collections::HashMap, fmt::Display};

#[cfg(feature = "c2d")]
use crate::messages::c2d::*;

#[cfg(feature = "direct-methods")]
use crate::messages::direct_methods::*;

#[cfg(feature = "twin")]
use crate::messages::twin::*;

#[cfg(feature = "telemetry")]
use crate::messages::telemetry::*;

use crate::messages::subscription::*;

/// Message properties data structure
pub type PropertyBag = HashMap<String, String>;

/// Represent a processing acknowledgement for the specified PacketId
#[derive(Clone, Debug, Copy)]
pub struct AckMsg {
    /// The acknowledged packet's identifier
    pub packet_id: PacketId,
}

/// Represents a single message from the IoT Hub to the device
#[derive(Clone, Debug)]
pub enum MsgFromHub {
    /// The codec did not recognize the decoded message.
    /// Example: the message is a C2D message, but the C2D feature was opted-out
    UnknownMessage(),

    /// The response to a connection attempt
    ConnectResponseMessage(ConnectRes),

    /// The response to a Twin Read request
    #[cfg(feature = "twin")]
    TwinResponseMessage(ReadTwinRes),

    /// An event representing an update to the twin's desired properties
    #[cfg(feature = "twin")]
    DesiredPropertiesUpdated(DesiredPropsUpdated),

    /// A C2D message
    #[cfg(feature = "c2d")]
    CloudToDeviceMessage(C2DMsg),

    /// A direct method invocation request
    #[cfg(feature = "direct-methods")]
    DirectMethodInvocation(DirectMethodReq),

    /// The response to a subscription request
    SubscriptionResponseMessage(SubRes),

    /// Publication acknowledgement
    PublicationSucceeded(PacketId),
}

impl Display for MsgFromHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            MsgFromHub::SubscriptionResponseMessage(resp) => match resp.result {
                Ok(()) => write!(f, "Subscription succeeded, packet id: {}", resp.packet_id),
                Err(_) => write!(f, "Subscription failed, packet id: {}", resp.packet_id),
            },
            MsgFromHub::PublicationSucceeded(packet_id) => {
                write!(f, "Publication succeeded: {}", packet_id)
            }
            #[cfg(feature = "twin")]
            MsgFromHub::TwinResponseMessage(resp) => write!(
                f,
                "Twin Response: {:?} {:?} {:?}",
                resp.status_code, resp.version, resp.body
            ),
            #[cfg(feature = "twin")]
            MsgFromHub::DesiredPropertiesUpdated(msg) => write!(
                f,
                "Desired properties updated, version: {}",
                msg.desired_properties_version
            ),
            #[cfg(feature = "c2d")]
            MsgFromHub::CloudToDeviceMessage(_msg) => write!(f, "C2D Msg"),
            #[cfg(feature = "direct-methods")]
            MsgFromHub::DirectMethodInvocation(dmi) => {
                write!(f, "Direct MEthod invocation, method: {}", dmi.method_name)
            }
            MsgFromHub::UnknownMessage() => write!(f, "Unknown msg"),
            _other => write!(f, "Some other msg"),
        }
    }
}

impl From<ConnectRes> for MsgFromHub {
    fn from(response: ConnectRes) -> Self {
        return MsgFromHub::ConnectResponseMessage(response);
    }
}

#[cfg(feature = "c2d")]
impl From<C2DMsg> for MsgFromHub {
    fn from(c2d: C2DMsg) -> Self {
        return MsgFromHub::CloudToDeviceMessage(c2d);
    }
}

#[cfg(feature = "twin")]
impl From<ReadTwinRes> for MsgFromHub {
    fn from(response: ReadTwinRes) -> Self {
        return MsgFromHub::TwinResponseMessage(response);
    }
}

#[cfg(feature = "twin")]
impl From<DesiredPropsUpdated> for MsgFromHub {
    fn from(update_notification: DesiredPropsUpdated) -> Self {
        return MsgFromHub::DesiredPropertiesUpdated(update_notification);
    }
}

#[cfg(feature = "direct-methods")]
impl From<DirectMethodReq> for MsgFromHub {
    fn from(invocation: DirectMethodReq) -> Self {
        return MsgFromHub::DirectMethodInvocation(invocation);
    }
}

impl From<SubRes> for MsgFromHub {
    fn from(response: SubRes) -> Self {
        return MsgFromHub::SubscriptionResponseMessage(response);
    }
}

/// Represnets a message from the device to the IoT hub
#[derive(Clone, Debug)]
pub enum MsgToHub {
    /// A connection attempt
    Connect(ConnectMsg),

    /// An ACK message (in response to an incoming message with QoS1)
    Acknowledge(AckMsg),

    /// A device-to-cloud telemetry message
    #[cfg(feature = "telemetry")]
    Telemetry(TelemetryMsg),

    /// A request to read the twin
    #[cfg(feature = "twin")]
    ReadTwin(ReadTwinReq),

    /// A request to receive twin read results.
    /// This subscription must be completed before sending a ReadTwin request.
    #[cfg(feature = "twin")]
    SubscribeToTwinReads(TwinReadSub),

    /// A request to receive twin reported properties update notifications
    #[cfg(feature = "twin")]
    SubscribeToTwinUpdates(TwinUpdatesSub),

    /// A notification about a change in the twin's reported properties
    #[cfg(feature = "twin")]
    UpdateReportedProperties(UpdateReportedPropsReq),

    /// A request to receive cloud-to-device messages
    #[cfg(feature = "c2d")]
    SubscribeToC2D(C2DSub),

    /// A request to receive direct method invocation requests
    #[cfg(feature = "direct-methods")]
    SubscribeToMethods(DirectMethodsSub),

    /// The result of a direct method invocation
    #[cfg(feature = "direct-methods")]
    DirectMethodResponse(DirectMethodRes),
}

impl MsgToHub {
    /// Return the packet ID of the specified message, if any
    pub fn packet_id(&self) -> Option<PacketId> {
        match self {
            MsgToHub::Connect(_) => None,

            MsgToHub::Acknowledge(msg) => Some(msg.packet_id),

            #[cfg(feature = "twin")]
            MsgToHub::ReadTwin(msg) => msg.packet_id,

            #[cfg(feature = "telemetry")]
            MsgToHub::Telemetry(msg) => msg.packet_id,

            #[cfg(feature = "c2d")]
            MsgToHub::SubscribeToC2D(msg) => Some(msg.packet_id),

            #[cfg(feature = "direct-methods")]
            MsgToHub::SubscribeToMethods(msg) => Some(msg.packet_id),

            #[cfg(feature = "direct-methods")]
            MsgToHub::DirectMethodResponse(msg) => msg.packet_id,

            #[cfg(feature = "twin")]
            MsgToHub::SubscribeToTwinReads(msg) => Some(msg.packet_id),

            #[cfg(feature = "twin")]
            MsgToHub::SubscribeToTwinUpdates(msg) => Some(msg.packet_id),

            #[cfg(feature = "twin")]
            MsgToHub::UpdateReportedProperties(msg) => msg.packet_id,
        }
    }
}

impl From<ConnectMsg> for MsgToHub {
    fn from(msg: ConnectMsg) -> Self {
        return MsgToHub::Connect(msg);
    }
}

impl From<AckMsg> for MsgToHub {
    fn from(msg: AckMsg) -> Self {
        return MsgToHub::Acknowledge(msg);
    }
}

#[cfg(feature = "telemetry")]
impl From<TelemetryMsg> for MsgToHub {
    fn from(msg: TelemetryMsg) -> Self {
        return MsgToHub::Telemetry(msg);
    }
}

#[cfg(feature = "twin")]
impl From<ReadTwinReq> for MsgToHub {
    fn from(msg: ReadTwinReq) -> Self {
        return MsgToHub::ReadTwin(msg);
    }
}

#[cfg(feature = "twin")]
impl From<TwinReadSub> for MsgToHub {
    fn from(msg: TwinReadSub) -> Self {
        return MsgToHub::SubscribeToTwinReads(msg);
    }
}

#[cfg(feature = "twin")]
impl From<TwinUpdatesSub> for MsgToHub {
    fn from(msg: TwinUpdatesSub) -> Self {
        return MsgToHub::SubscribeToTwinUpdates(msg);
    }
}

#[cfg(feature = "twin")]
impl From<UpdateReportedPropsReq> for MsgToHub {
    fn from(msg: UpdateReportedPropsReq) -> Self {
        return MsgToHub::UpdateReportedProperties(msg);
    }
}

#[cfg(feature = "c2d")]
impl From<C2DSub> for MsgToHub {
    fn from(msg: C2DSub) -> Self {
        return MsgToHub::SubscribeToC2D(msg);
    }
}

#[cfg(feature = "direct-methods")]
impl From<DirectMethodsSub> for MsgToHub {
    fn from(msg: DirectMethodsSub) -> Self {
        return MsgToHub::SubscribeToMethods(msg);
    }
}

#[cfg(feature = "direct-methods")]
impl From<DirectMethodRes> for MsgToHub {
    fn from(msg: DirectMethodRes) -> Self {
        return MsgToHub::DirectMethodResponse(msg);
    }
}
