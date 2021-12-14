use crate::{identity::ClientIdentity, qos::SessionMode};
use core::fmt::{self, Display};

/// A request to connect to the IoT Hub
#[derive(Clone, Debug)]
pub struct ConnectMsg {
    /// The identity of the client (device or module)
    pub client_id: ClientIdentity,

    /// The address of the server (foamrt is "host:port")
    pub server_addr: String,

    /// A SAS token used for auth
    /// Not required if the client identifies using an x509 certificate
    pub sas_token: Option<String>,

    /// The session mode of the new connection
    pub session_mode: SessionMode,
}

/// Represents the IoT Hub's response to the connection request
#[derive(Clone, Debug, Copy)]
pub enum ConnectRes {
    /// The connection succeeded
    Accepted,

    /// Authentication failure (i.e. incorrect credentials, or device is not defined in this hub, etc.)
    AuthenticationFailed,

    /// The device is not authorized to connect to the hub
    Unauthorized,

    /// The IoT Hub service is unavailable
    ServiceUnavailable,

    /// The connection attempt timed out
    Timeout,

    /// MQTT protocol version is unacceptable
    UnacceptableProtocolVersion,

    /// MQTT reserved error code
    MqttReservedErrorCode(u8),

    /// Unexpected message or message flow
    ProtocolViolation,

    /// IO Error
    IOError(std::io::ErrorKind),
}

impl Display for ConnectRes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
