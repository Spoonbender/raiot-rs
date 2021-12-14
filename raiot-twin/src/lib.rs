extern crate mqtt;
extern crate native_tls;
extern crate serde;
extern crate serde_json;
extern crate uuid;
extern crate raiot_types;
extern crate raiot_codec;

use raiot_codec::iot_codec::IotCodec;
use serde_json::Value;
use std::collections::HashMap;
use native_tls::{TlsStream};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::net::TcpStream;
use std::str;

use uuid::Uuid;

use mqtt::packet::{
    Packet, VariablePacket, PacketError
};
use mqtt::{Decodable};

use raiot_types::*;
use raiot_types::twin::*;
use raiot_codec::*;
use raiot_types::StatusCode::*;
use raiot_types::MessageFromServer::*;

/// Errors while parsing topic names
#[derive(Debug)]
pub enum DeviceTwinError {
    //FeatureNotSupported(),
    BadRequest(),
    Failure(String),
    ServerError(u16),
    UnknownError(u16),
    TooManyRequests(),
    IoError(io::Error)
}

impl fmt::Display for DeviceTwinError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            //&DeviceTwinError::FeatureNotSupported() => write!(f, "Device Twins are not supported"),
            &DeviceTwinError::Failure(ref error) => write!(f, "Failure getting device twin ({})", error),
            &DeviceTwinError::TooManyRequests() => write!(f, "Too many requests (throttled)"),
            &DeviceTwinError::ServerError(error_code) => write!(f, "Server Error {}", error_code),
            &DeviceTwinError::IoError(ref io_error) => io_error.fmt(f),
            &DeviceTwinError::BadRequest() => write!(f, "Bad request"),
            &DeviceTwinError::UnknownError(error_code) => write!(f, "Unknown error {}", error_code),          
        }
    }
}

impl Error for DeviceTwinError {
    fn description(&self) -> &str {
        match self {
            //&DeviceTwinError::FeatureNotSupported() => "Device Twins are not supported for this IoT Hub",
            &DeviceTwinError::ServerError(_) => "Server error occured",
            &DeviceTwinError::TooManyRequests() => "Too many requests (throttled)",
            &DeviceTwinError::Failure(ref error) => error,
            &DeviceTwinError::IoError(ref io_error) => io_error.description(),
            &DeviceTwinError::BadRequest() => "Bad request",
            &DeviceTwinError::UnknownError(_error_code) => "Unknown error",    
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        match self {
            //&DeviceTwinError::FeatureNotSupported() => None,
            &DeviceTwinError::Failure(ref _error) => None,
            _ => None,
        }
    }
}

impl From<io::Error> for DeviceTwinError {
    fn from(item: io::Error) -> Self {
        DeviceTwinError::Failure(format!("I/O Error occured: {}", item.description()))
    }
}

impl<T: Packet> From<PacketError<T>> for DeviceTwinError {
    fn from(item: PacketError<T>) -> Self {
        return match item {
            PacketError::IoError(io_err) => DeviceTwinError::IoError(io_err),
            _ => DeviceTwinError::Failure(format!("Packet Error occured")) // TODO proper error propagation
        }
    }
}

pub fn update_reported_properties(mut stream: &mut TlsStream<TcpStream>, reported: HashMap<String, Value>) -> Result<(), DeviceTwinError> {
    let codec = IotCodec; //TODO
    let rid = Uuid::new_v4();
    let buf = codec.encode_twin_update(rid, reported, Option::Some(10)).unwrap();
    
    stream.write_all(&buf[..])?;

    let packet2 = match VariablePacket::decode(&mut stream) {
        Ok(pk) => pk,
        Err(err) => {
            debug!("Error in receiving packet {}", err);
            return Err(DeviceTwinError::Failure(
                "Unexpected response packet".to_string(),
            ));
        }
    };
    debug!("PACKET after update {:?}", packet2);

    return match packet2 {
        VariablePacket::PublishPacket(ref _publ) => Ok(()),
        _ => Err(DeviceTwinError::Failure("Unexpected response packet".to_string()))
    };
}

pub fn read_twin(mut stream: &mut TlsStream<TcpStream>) -> Result<Twin, DeviceTwinError> {
    let codec = IotCodec; //TODO
    subscribe_to_twin_result(&mut stream)?;
    
    let packet = codec.decode(stream);
    match packet {
        Ok(SubscriptionSucceeded(pkid)) => { },
        Ok(other) => { panic!("unexpected response") }
        Err(e) => panic!(e)
    };

    let rid = Uuid::new_v4();
    let buf = codec.encode_read_twin(rid, Option::None).unwrap();   
    stream.write_all(&buf[..])?;

    let packet2 = codec.decode(stream);
    let packet2 = packet2.unwrap();

    return match packet2 {
        MessageFromServer::TwinResponseMessage(ref resp) => parse_twin_packet(resp),
        _ => Err(DeviceTwinError::Failure("Unexpected response packet".to_string()))
    };
}

fn subscribe_to_twin_result(stream: &mut TlsStream<TcpStream>) -> Result<(), DeviceTwinError> {
    let codec = IotCodec; //TODO
    let buf = codec.encode_twin_subscription(10).unwrap();
    stream.write_all(&buf[..])?;
    Ok(())
}

fn parse_twin_packet(packet: &TwinResponse) -> Result<Twin, DeviceTwinError> {
    return match packet.status_code() {
        StatusCode::OK() => Ok(Twin::new(&packet.body.as_ref().unwrap())),
        other => Err(error_code_to_twin_error(other))
    };
}

fn error_code_to_twin_error(code: &StatusCode) -> DeviceTwinError {
    return match code {
        TooManyRequests() => DeviceTwinError::TooManyRequests(),
        BadRequest() => DeviceTwinError::BadRequest(),
        ServerError(error_code) => DeviceTwinError::ServerError(*error_code),
        UnknownStatusCode(error_code) => DeviceTwinError::UnknownError(*error_code),
        OK() => panic!("Success was passed to error_code_to_twin_error"),
        NoContent() => panic!("Success was passed to error_code_to_twin_error")
    };
}