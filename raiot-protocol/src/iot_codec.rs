use mqtt;
use serde::Deserialize;
use serde_json;

use crate::messages::{MsgFromHub, MsgToHub};
use crate::*;
use crate::{connect::ConnectMsg, connect::ConnectRes, messages::MsgFromHub::PublicationSucceeded};
use log::debug;
use messages::AckMsg;
use mqtt::control::variable_header::ConnectReturnCode;
use mqtt::packet::suback::SubscribeReturnCode;
use mqtt::packet::*;
use mqtt::Decodable;
use mqtt::{Encodable, QualityOfService, TopicFilter, TopicName};
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use qos::{DeliveryGuarantees, PacketId, SessionMode};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use subscription::SubRes;
use url::{form_urlencoded, Url};

#[cfg(feature = "c2d")]
use messages::c2d::{C2DMsg, C2DSub};

#[cfg(feature = "direct-methods")]
use messages::direct_methods::{DirectMethodReq, DirectMethodRes, DirectMethodsSub};

#[cfg(feature = "twin")]
use messages::twin::*;

#[cfg(feature = "telemetry")]
use messages::telemetry::TelemetryMsg;

/// A Codec that ebcodes and decodes between IoT messages and MQTT packets
#[derive(Debug, Copy, Clone)]
pub struct IotCodec;

/// The result of an encoding process
pub type EncodingResult = Result<usize, CodecError>;

/// The result of a decoding process
pub type DecodingResult = Result<MsgFromHub, CodecError>;

/// Represents an error in encoding or decoding a packet
#[derive(Debug, Copy, Clone)]
pub enum CodecError {
    /// The MQTT packet type is unknown or unexpected
    UnexpectedMqttPacketType,

    /// The MQTT packet is invalid, according to the MQTT spec
    InvalidMqttPacket,

    /// The message body does not match the Azure IoT Hub MQTT spec
    InvalidMessageBody,

    /// The topic name is invalid, according to the Azure IoT Hub MQTT spec
    InvalidTopic,

    /// The Request ID is missing
    MissingRid,

    /// The topic name lacks the device ID
    MissingDeviceId,

    /// The direct method invocation packet is missing the invoked method name
    #[cfg(feature = "direct-methods")]
    MissingMethodName,

    /// The twin version indicator is missing
    #[cfg(feature = "twin")]
    MissingVersion,

    /// The twin operation status code is missing
    #[cfg(feature = "twin")]
    MissingStatusCode,

    /// The twin version identifier is invalid
    #[cfg(feature = "twin")]
    InvalidVersionIdentifier,
}


/// Messages which can be encoded to MQTT
pub trait MqttEncodable {
    /// Encodes the message to MQTT
    fn encode(&self) -> VariablePacket;
}

impl MqttEncodable for ConnectMsg {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_connect_message(&self).into()
    }
}

impl MqttEncodable for AckMsg {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_ack_message(&self).into()
    }
}

#[cfg(feature = "telemetry")]
impl MqttEncodable for TelemetryMsg {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_telemetry_message(&self).into()
    }
}

#[cfg(feature = "c2d")]
impl MqttEncodable for C2DSub {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_c2d_messages_subscription(&self).into()
    }
}

#[cfg(feature = "direct-methods")]
impl MqttEncodable for DirectMethodsSub {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_c2d_methods_subscription(&self).into()
    }
}

#[cfg(feature = "direct-methods")]
impl MqttEncodable for DirectMethodRes {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_direct_method_response(&self).into()
    }
}

#[cfg(feature = "twin")]
impl MqttEncodable for ReadTwinReq {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_read_twin(&self).into()
    }
}

#[cfg(feature = "twin")]
impl MqttEncodable for TwinReadSub {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_twin_subscription(&self).into()
    }
}

#[cfg(feature = "twin")]
impl MqttEncodable for TwinUpdatesSub {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_twin_updates_subscription(&self).into()
    }
}

#[cfg(feature = "twin")]
impl MqttEncodable for UpdateReportedPropsReq {
    fn encode(&self) -> VariablePacket {
        IotCodec::encode_twin_update(&self).into()
    }
}


impl CodecError {
    fn get_text<'a>(self: &'a CodecError) -> &'a str {
        match self {
            CodecError::UnexpectedMqttPacketType => "Unexpected MQTT Packet Type",
            CodecError::InvalidMqttPacket => "Invalid MQTT Packet",
            CodecError::InvalidMessageBody => "Invalid Message Body",
            CodecError::InvalidTopic => "Invalid Topic",
            CodecError::MissingRid => "Missing Request IDentifier (RID)",
            CodecError::MissingDeviceId => "Missing Device ID",
            #[cfg(feature = "direct-methods")]
            CodecError::MissingMethodName => "Missing Direct Method Name",
            #[cfg(feature = "twin")]
            CodecError::MissingVersion => "Missing Version Identifier",
            #[cfg(feature = "twin")]
            CodecError::MissingStatusCode => "Missing Status Code",
            #[cfg(feature = "twin")]
            CodecError::InvalidVersionIdentifier => "Invalid Twin Version Identifier",
        }
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return write!(f, "{}", &self.get_text());
    }
}

impl Error for CodecError {
    fn description(&self) -> &str {
        return "Codec failure";
    }

    fn cause(&self) -> Option<&dyn Error> {
        return None;
    }
}

impl IotCodec {
    /// Encodes a MsgToHub into the provided buffer. Returns the encoded message size, or an error.
    ///
    /// # Arguments
    ///
    /// * message - the IoT message to encode
    /// * buf - a buffer into which the message will be encoded
    ///
    /// # Panics
    /// Panics if the message was encoded to an MQTT packet successfully, but could not be encoded to bytes
    pub fn encode(message: &MsgToHub, mut buf: &mut [u8]) -> EncodingResult {
        let packet = Self::encode_message(message)?;
        packet.encode(&mut buf).unwrap();
        let length = packet.encoded_length();
        return Ok(length as usize);
    }

    /// Decodes a single message from hub from the provided buffer
    ///
    /// # Errors
    /// Returns an error if the buffer contains an invalid MQTT packet, or if the MQTT packet translates to an invalid IoT Hub packet
    pub fn decode(bytes: &[u8]) -> DecodingResult {
        let mut buf = &bytes[..];
        let decode_res = VariablePacket::decode(&mut buf);
        
        decode_res
            .map_err(|_e| CodecError::InvalidMqttPacket)
            .map(|packet| Self::decode_packet(packet.into()))?
    }

    /// Encodes an IoT message to an MQTT packet
    pub fn encode_message(message: &MsgToHub) -> Result<VariablePacket, CodecError> {
        let encoded: VariablePacket = match message {
            MsgToHub::Connect(ref msg) => Self::encode_connect_message(&msg).into(),

            MsgToHub::Acknowledge(ref msg) => Self::encode_ack_message(&msg).into(),

            #[cfg(feature = "twin")]
            MsgToHub::ReadTwin(ref msg) => Self::encode_read_twin(&msg).into(),

            #[cfg(feature = "telemetry")]
            MsgToHub::Telemetry(ref msg) => Self::encode_telemetry_message(&msg).into(),

            #[cfg(feature = "c2d")]
            MsgToHub::SubscribeToC2D(ref msg) => {
                Self::encode_c2d_messages_subscription(&msg).into()
            }

            #[cfg(feature = "direct-methods")]
            MsgToHub::SubscribeToMethods(ref msg) => {
                Self::encode_c2d_methods_subscription(&msg).into()
            }

            #[cfg(feature = "twin")]
            MsgToHub::SubscribeToTwinReads(ref msg) => Self::encode_twin_subscription(&msg).into(),

            #[cfg(feature = "direct-methods")]
            MsgToHub::DirectMethodResponse(ref msg) => {
                Self::encode_direct_method_response(&msg).into()
            }

            #[cfg(feature = "twin")]
            MsgToHub::SubscribeToTwinUpdates(ref msg) => {
                Self::encode_twin_updates_subscription(&msg).into()
            }

            #[cfg(feature = "twin")]
            MsgToHub::UpdateReportedProperties(ref msg) => Self::encode_twin_update(&msg).into(),
        };

        Ok(encoded)
    }

    /// Decodes an MQTT packet into an IoT packet. Returns the IoT message, or an error.
    ///
    /// # Arguments
    ///
    /// * packet - the MQTT packet to decode
    pub fn decode_packet(packet: VariablePacket) -> DecodingResult {
        return match packet {
            VariablePacket::ConnackPacket(ref connack) => Self::decode_connack_packet(connack),
            VariablePacket::PublishPacket(ref publ) => Self::decode_publish_packet(publ),
            VariablePacket::PubackPacket(ref puback) => Self::decode_puback_packet(puback),
            VariablePacket::SubackPacket(ref suback) => Self::decode_suback_packet(suback),
            _other_packet => Err(CodecError::UnexpectedMqttPacketType),
        };
    }

    fn decode_connack_packet(packet: &ConnackPacket) -> DecodingResult {
        let resp = match packet.connect_return_code() {
            ConnectReturnCode::ConnectionAccepted => ConnectRes::Accepted,
            ConnectReturnCode::BadUserNameOrPassword => ConnectRes::AuthenticationFailed,
            ConnectReturnCode::ServiceUnavailable => ConnectRes::ServiceUnavailable,
            ConnectReturnCode::UnacceptableProtocolVersion => {
                ConnectRes::UnacceptableProtocolVersion
            }
            ConnectReturnCode::IdentifierRejected => ConnectRes::AuthenticationFailed,
            ConnectReturnCode::NotAuthorized => ConnectRes::Unauthorized,
            ConnectReturnCode::Reserved(code) => ConnectRes::MqttReservedErrorCode(code),
        };

        Ok(MsgFromHub::ConnectResponseMessage(resp))
    }

    fn decode_suback_packet(packet: &SubackPacket) -> DecodingResult {
        Ok(SubRes {
            packet_id: packet.packet_identifier().into(),
            result: match packet.payload_ref().subscribes()[0] {
                SubscribeReturnCode::Failure => Err(SubError::Failure),
                _other => Ok(()),
            },
        }
        .into())
    }

    fn decode_puback_packet(packet: &PubackPacket) -> DecodingResult {
        Ok(PublicationSucceeded(packet.packet_identifier().into()))
    }

    fn decode_publish_packet(packet: &PublishPacket) -> DecodingResult {
        // TODO improve algorithm performance (better branching)
        // TODO pass rest of topic to decode method
        #[cfg(feature = "twin")]
        if packet.topic_name().starts_with("$iothub/twin/res/") {
            return Self::decode_twin_response(packet);
        }

        #[cfg(feature = "twin")]
        if packet
            .topic_name()
            .starts_with("$iothub/twin/PATCH/properties/desired/")
        {
            return Self::decode_desired_properties_update(packet);
        }

        #[cfg(feature = "direct-methods")]
        if packet.topic_name().starts_with("$iothub/methods/POST/") {
            return Self::decode_direct_method_invocation(packet);
        }

        #[cfg(feature = "c2d")]
        if packet.topic_name().starts_with("devices/") {
            return Self::decode_c2d_message(packet);
        }

        return Ok(MsgFromHub::UnknownMessage());
    }

    fn encode_ack_message(msg: &AckMsg) -> PubackPacket {
        PubackPacket::new(msg.packet_id.into())
    }

    fn encode_connect_message(msg: &ConnectMsg) -> ConnectPacket {
        let client_identifier = match &msg.client_id {
            ClientIdentity::Device(device) => device.device_id.clone(),
            ClientIdentity::Module(module) => format!("{}/{}", module.device_id, module.module_id),
        };

        let mut packet = ConnectPacket::new(&client_identifier);
        match msg.session_mode {
            SessionMode::Clean => packet.set_clean_session(true),
            SessionMode::Dirty => packet.set_clean_session(false),
        };

        let username = match &msg.client_id {
            ClientIdentity::Device(device) => format!(
                "{}/{}/api-version=2018-06-30",
                msg.server_addr, device.device_id
            ),
            ClientIdentity::Module(module) => format!(
                "{}/{}/{}/api-version=2018-06-30",
                msg.server_addr, module.device_id, module.module_id
            ),
        };
        packet.set_user_name(Some(username));
        if let Some(ref token) = msg.sas_token {
            packet.set_password(Some(token.to_owned()));
        }
        return packet;
    }

    #[cfg(feature = "c2d")]
    fn decode_c2d_message(packet: &PublishPacket) -> DecodingResult {
        let body = deserialize_message_body(&packet)?;

        let topic = packet.topic_name();
        debug!("C2D Topic name: {:?}", topic);

        let mut segments = topic.split('/');
        if let None = segments.next() {
            return Err(CodecError::InvalidTopic);
        }

        let device_id = match segments.next() {
            Some(id) => id.to_owned(),
            None => return Err(CodecError::MissingDeviceId),
        };

        let mut props: Option<HashMap<String, String>> = None;
        if let Some(value) = segments.skip(2).next() {
            let vals: HashMap<String, String> = form_urlencoded::parse(value.as_bytes())
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            props = Some(vals);
        }

        let packet_id = qos_to_packet_id(packet.qos());

        let message = C2DMsg {
            packet_id,
            body,
            device_id,
            props,
        };

        Ok(message.into())
    }

    #[cfg(feature = "direct-methods")]
    fn decode_direct_method_invocation(packet: &PublishPacket) -> DecodingResult {
        let topic = packet.topic_name();
        let parsed_url = Url::parse(&("mqtt://".to_owned() + topic)).unwrap();
        let mut hash_query: HashMap<_, _> = parsed_url.query_pairs().into_owned().collect();
        let request_id = 
            hash_query.remove("$rid")
            .ok_or_else(|| CodecError::MissingRid.into())?;
        let body = deserialize_message_body(&packet)?;
        let mut segments = parsed_url.path_segments().unwrap();
        if let None = segments.next() {
            return Err(CodecError::InvalidTopic);
        }
        if let None = segments.next() {
            return Err(CodecError::InvalidTopic);
        }

        let method_name = match segments.next() {
            Some(name) => percent_decode_str(name).decode_utf8().unwrap().into_owned(),
            None => return Err(CodecError::MissingMethodName),
        };

        let message = DirectMethodReq {
            body,
            request_id,
            method_name,
            packet_id: qos_to_packet_id(packet.qos()),
        };

        Ok(message.into())
    }

    #[cfg(feature = "twin")]
    fn decode_desired_properties_update(packet: &PublishPacket) -> DecodingResult {
        let topic = packet.topic_name();
        let parsed_url = Url::parse(&("mqtt://".to_owned() + topic)).unwrap();
        let hash_query: HashMap<_, _> = parsed_url.query_pairs().into_owned().collect();
        let version = match hash_query.get("$version") {
            Some(version) => version,
            None => return Err(CodecError::MissingVersion),
        };
        let version = version.parse::<u64>().map_err(|_e| CodecError::InvalidVersionIdentifier.into())?;
        let body = 
            deserialize_message_body(&packet)?
            .ok_or_else(|| CodecError::InvalidMessageBody)?;

        let message = DesiredPropsUpdated {
            packet_id: qos_to_packet_id(packet.qos()),
            body: body,
            desired_properties_version: version,
        };

        Ok(message.into())
    }

    #[cfg(feature = "twin")]
    fn decode_twin_response(packet: &PublishPacket) -> DecodingResult {
        let topic = packet.topic_name();

        let parsed_url = Url::parse(&("mqtt://".to_owned() + topic)).unwrap();
        let mut hash_query: HashMap<_, _> = parsed_url.query_pairs().into_owned().collect();
        let rid = match hash_query.remove("$rid") {
            Some(rid) => rid,
            None => return Err(CodecError::MissingRid),
        };

        // TODO slicing is the wrong way to go as it might provide an invalid string... 
        match topic[17..20].parse::<u16>() {
            Err(_) => return Err(CodecError::MissingStatusCode),
            Ok(code) => {
                let body = match code {
                    200 => deserialize_message_body(&packet)?,
                    _other => None,
                };

                return Ok(MsgFromHub::TwinResponseMessage(ReadTwinRes {
                    packet_id: qos_to_packet_id(packet.qos()),
                    request_id: rid,
                    status_code: Self::get_status_code(code), // TODO move out of "self" ?
                    body: body,
                    version: Self::extract_version(&hash_query)?,
                }));
            }
        }
    }

    #[cfg(feature = "twin")]
    fn extract_version(map: &HashMap<String, String>) -> Result<Option<u64>, CodecError> {
        let version = match map.get("$version") {
            Some(version_string) => {
                return match version_string.parse::<u64>() {
                    Ok(version) => Ok(Some(version)),
                    Err(_e) => Err(CodecError::InvalidVersionIdentifier),
                };
            }
            None => Ok(None),
        };

        return version;
    }

    #[cfg(feature = "twin")]
    fn get_status_code(code: u16) -> StatusCode {
        return match code {
            429 => StatusCode::TooManyRequests(),
            200 => StatusCode::OK(),
            204 => StatusCode::NoContent(),
            500..=599 => StatusCode::ServerError(code),
            other => StatusCode::UnknownStatusCode(other),
        };
    }

    #[cfg(feature = "telemetry")]
    fn encode_telemetry_message(message: &TelemetryMsg) -> PublishPacket {
        let qos_and_id = packet_id_to_qos(message.packet_id);

        let mut channel = match &message.client_id {
            ClientIdentity::Device(device) => {
                format!("devices/{}/messages/events/", device.device_id)
            }
            ClientIdentity::Module(module) => format!(
                "devices/{}/modules/{}/messages/events/",
                module.device_id, module.module_id
            ),
        };

        if let Some(headers) = &message.headers {
            // TODO there has to be a built-in way to do this thing...
            let mut bag = String::new();
            let mut first = true;
            for (key, value) in headers {
                if !first {
                    bag.push('&');
                }
                let encoded_key = utf8_percent_encode(key, NON_ALPHANUMERIC).to_string();
                bag.push_str(&encoded_key);
                bag.push('=');
                let encoded_value = utf8_percent_encode(value, NON_ALPHANUMERIC).to_string();
                bag.push_str(&encoded_value);
                first = false;
            }
            channel.push_str(&bag);
        }

        let channel = TopicName::new(channel).expect("Topic name must be valid");
        let payload = match &message.content {
            Some(value) => value.to_string().into_bytes(),
            None => Vec::new(),
        };
        let publish_packet = PublishPacket::new(channel, qos_and_id, payload);
        return publish_packet;
    }

    #[cfg(feature = "twin")]
    fn encode_twin_update(message: &UpdateReportedPropsReq) -> PublishPacket {
        let qos_and_id = packet_id_to_qos(message.packet_id);
        let payload = serde_json::to_string(&message.reported).unwrap();
        let topic_name = format!(
            "$iothub/twin/PATCH/properties/reported/?$rid={}",
            message.request_id
        );
        let chan = TopicName::new(topic_name).unwrap(); // TODO
        let packet = PublishPacket::new(chan, qos_and_id, payload);
        return packet;
    }

    #[cfg(feature = "twin")]
    fn encode_read_twin(message: &ReadTwinReq) -> PublishPacket {
        let chan =
            TopicName::new(format!("$iothub/twin/GET/?$rid={}", message.request_id)).unwrap(); // TODO
        let qos_and_id = packet_id_to_qos(message.packet_id);
        let publish_packet = PublishPacket::new(chan, qos_and_id, Vec::new());
        return publish_packet;
    }

    #[cfg(feature = "twin")]
    fn encode_twin_subscription(message: &TwinReadSub) -> SubscribePacket {
        let topic_filter = "$iothub/twin/res/#";
        return Self::encode_subscription(message.packet_id.into(), topic_filter, message.mode);
    }

    #[cfg(feature = "twin")]
    fn encode_twin_updates_subscription(message: &TwinUpdatesSub) -> SubscribePacket {
        let topic_filter = "$iothub/twin/PATCH/properties/desired/#";
        return Self::encode_subscription(message.packet_id.into(), topic_filter, message.mode);
    }

    #[cfg(feature = "c2d")]
    fn encode_c2d_messages_subscription(message: &C2DSub) -> SubscribePacket {
        let topic_filter = &format!("devices/{}/messages/devicebound/#", message.device_id);
        Self::encode_subscription(message.packet_id, topic_filter, message.mode)
    }

    #[cfg(feature = "direct-methods")]
    fn encode_c2d_methods_subscription(message: &DirectMethodsSub) -> SubscribePacket {
        return Self::encode_subscription(
            message.packet_id,
            "$iothub/methods/POST/#",
            message.mode,
        );
    }

    fn encode_subscription(
        packet_id: PacketId,
        topic_filter: &str,
        mode: DeliveryGuarantees,
    ) -> SubscribePacket {
        let qos = match mode {
            DeliveryGuarantees::AtLeastOnce => QualityOfService::Level1,
            DeliveryGuarantees::AtMostOnce => QualityOfService::Level0,
        };

        let filter: (TopicFilter, QualityOfService) =
            (TopicFilter::new(topic_filter).unwrap(), qos);

        let mut filters: Vec<(TopicFilter, QualityOfService)> = Vec::new();
        filters.push(filter);

        debug!("Encoded sub {:?}", packet_id);
        return SubscribePacket::new(packet_id.into(), filters);
    }

    #[cfg(feature = "direct-methods")]
    fn encode_direct_method_response(message: &DirectMethodRes) -> PublishPacket {
        let topic_name = format!(
            "$iothub/methods/res/{}/?$rid={}",
            message.status, message.request_id
        );
        let topic_name = TopicName::new(topic_name).expect("Topic name must be legal");

        let payload = match &message.payload {
            Some(x) => x.to_string(),
            None => "".to_owned(),
        };

        let qos = packet_id_to_qos(message.packet_id);

        let packet = PublishPacket::new(topic_name, qos, payload);
        return packet;
    }
}

fn qos_to_packet_id(qos: QoSWithPacketIdentifier) -> Option<PacketId> {
    match qos {
        QoSWithPacketIdentifier::Level0 => None,
        QoSWithPacketIdentifier::Level1(pkid) => Some(pkid.into()),
        QoSWithPacketIdentifier::Level2(pkid) => Some(pkid.into()),
    }
}

fn packet_id_to_qos(packet_id: Option<PacketId>) -> QoSWithPacketIdentifier {
    match packet_id {
        Some(id) => QoSWithPacketIdentifier::Level1(id.into()),
        None => QoSWithPacketIdentifier::Level0,
    }
}

#[cfg(any(feature = "twin", feature = "c2d", feature = "direct-methods"))]
fn deserialize_message_body<'packet, T>(packet: &'packet PublishPacket) -> Result<Option<T>, CodecError> where T: Deserialize<'packet> {
    let json_result = serde_json::from_slice(packet.payload_ref());
    match json_result {
        Ok(json) => Ok(json),
        Err(_e) => Err(CodecError::InvalidMessageBody),
    }
}