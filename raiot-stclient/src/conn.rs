use std::{io::ErrorKind, time::Instant};

use mqtt::{control::ConnectReturnCode, packet::VariablePacket};
use raiot_client_base::{generate_sas_token, ConnectionSettings, PacketsNumerator};
use raiot_mqtt::connection::{MqttConnectError, MqttConnectionInProgress, MqttConnector};
use raiot_protocol::{auth::DeviceCredentials, connect::ConnectMsg, ClientIdentity, IotCodec};
use raiot_streams::{open_nonblocking_stream, ClientCertificate};

use crate::{sub::SubState, IotClient, MyStream};

pub enum IotConnState {
    Connected(IotClient),
    Connecting(IotConnectionInProgress),
    ConnectFailed(ConnectReturnCode), // TODO encapsulate
}

pub struct IotConnectionInProgress {
    connection: MqttConnectionInProgress<MyStream>,
    client_id: ClientIdentity,
}

impl IotConnectionInProgress {
    pub fn complete(self) -> std::io::Result<IotConnState> {
        match self.connection.complete() {
            Ok(connection) => Ok(IotConnState::Connected(IotClient {
                connection,
                client_id: self.client_id,
                packets_numerator: PacketsNumerator::new(),
                twin_read: SubState::Unsubscribed,
                dmi: SubState::Unsubscribed,
                twin_updates: SubState::Unsubscribed,
                c2d: SubState::Unsubscribed,
            })),
            Err(MqttConnectError::IOError(kind)) => Err(kind.into()),
            Err(MqttConnectError::WouldBlock(connection)) => {
                Ok(IotConnState::Connecting(IotConnectionInProgress {
                    connection,
                    client_id: self.client_id,
                }))
            }
            Err(MqttConnectError::ConnectFailed(rc)) => Ok(IotConnState::ConnectFailed(rc)),
            Err(MqttConnectError::ProtocolViolation) => Err(ErrorKind::InvalidData.into()),
        }
    }
}

impl IotClient {
    pub fn connect(settings: &ConnectionSettings) -> std::io::Result<IotConnectionInProgress> {
        let now = Instant::now();

        let client_certificate = match settings.credentials {
            DeviceCredentials::Certificate(ref cert) => Some(ClientCertificate {
                bytes: cert.bytes.clone(),
                password: cert.password.clone(),
            }),
            DeviceCredentials::Sas(_) => None,
        };

        let stream = open_nonblocking_stream(
            &settings.hostname,
            settings.port.into(),
            settings.timeout,
            client_certificate.as_ref(),
        )?
        .inner();

        let token = match settings.credentials {
            DeviceCredentials::Sas(ref key) => Some(generate_sas_token(settings, key).into()),
            DeviceCredentials::Certificate(_) => None,
        };

        let conn = ConnectMsg {
            client_id: settings.client_id.clone(),
            server_addr: settings.hostname.clone(),
            sas_token: token,
            session_mode: settings.session_mode,
        };

        let connpack = IotCodec::encode_message(&conn.into()).unwrap();
        let connpack = match connpack {
            VariablePacket::ConnectPacket(p) => p,
            _ => panic!("wat"),
        };

        let connection = MqttConnector::create(stream)
            .with_timeout(settings.timeout - now.elapsed())
            .connect(connpack)?;

        Ok(IotConnectionInProgress {
            connection,
            client_id: settings.client_id.clone(),
        })
    }
}
