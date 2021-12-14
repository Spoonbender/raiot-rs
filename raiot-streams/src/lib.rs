use std::io::{ErrorKind, Read, Write};
use std::iter::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

use std::error::Error;

#[cfg(feature = "use-native-tls")]
extern crate native_tls;

#[cfg(feature = "use-native-tls")]
use native_tls::{HandshakeError, Identity, MidHandshakeTlsStream, TlsConnector, TlsStream};

#[derive(Clone, Debug)]
pub struct ClientCertificate {
    pub bytes: Vec<u8>,
    pub password: String,
}

#[cfg(feature = "use-native-tls")]
pub struct IoStream {
    stream: TlsStream<TcpStream>,
}

impl IoStream {
    pub fn inner(self) -> TlsStream<TcpStream> {
        self.stream
    }
}

#[macro_use]
extern crate log;

#[cfg(feature = "use-native-tls")]
pub fn open_stream(
    server_addr: &str,
    server_port: u32,
    timeout: Duration,
) -> Result<IoStream, std::io::Error> {
    let stream = open_tcp_stream(server_addr, server_port, timeout)?;
    let stream = open_tls_stream(server_addr, stream);
    Ok(IoStream { stream: stream })
}

#[cfg(feature = "use-native-tls")]
pub fn open_nonblocking_stream(
    server_addr: &str,
    server_port: u32,
    timeout: Duration,
    client_certificate: Option<&ClientCertificate>,
) -> Result<IoStream, std::io::Error> {
    assert!(timeout > Duration::from_millis(0));
    let now = Instant::now();
    let stream = open_tcp_stream(server_addr, server_port, timeout)?;
    stream.set_nonblocking(true)?;
    let timeout = timeout - now.elapsed();
    let stream = open_nonblocking_tls_stream(server_addr, stream, timeout, client_certificate)?;

    debug!("NonBlocking stream opened");

    Ok(IoStream { stream: stream })
}

fn open_tcp_stream(
    server_addr: &str,
    server_port: u32,
    timeout: Duration,
) -> Result<TcpStream, std::io::Error> {
    let server_socket = format!("{}:{}", server_addr, server_port);

    let addr = server_socket.to_socket_addrs()?.next().unwrap();

    debug!("Connecting TCP stream to {:?} ... ", server_socket);
    let stream = TcpStream::connect_timeout(&addr, timeout)?;

    stream.set_read_timeout(Option::Some(std::time::Duration::from_millis(1000)))?;

    debug!("TCP Connected!");

    Ok(stream)
}

#[cfg(feature = "use-native-tls")]
fn open_tls_stream(server_addr: &str, inner_stream: TcpStream) -> TlsStream<TcpStream> {
    debug!("Connecting TLS...");
    let connector = TlsConnector::new().unwrap();
    let stream = connector.connect(server_addr, inner_stream).unwrap();
    debug!("TLS Connected!");
    return stream;
}

#[cfg(feature = "use-native-tls")]
fn open_nonblocking_tls_stream(
    server_addr: &str,
    inner_stream: TcpStream,
    timeout: Duration,
    client_certificate: Option<&ClientCertificate>,
) -> Result<TlsStream<TcpStream>, std::io::Error> {
    debug!("Connecting TLS...");

    let mut builder = TlsConnector::builder();
    if let Some(cert) = client_certificate {
        builder.identity(Identity::from_pkcs12(&cert.bytes, &cert.password).unwrap());
    }

    let connector = builder.build().unwrap();

    match connector.connect(&server_addr, inner_stream) {
        Ok(tls_stream) => return Ok(tls_stream),
        Err(HandshakeError::WouldBlock(tls_stream)) => {
            trace!("Socket is not ready, backing off for a bit...");
            std::thread::sleep(std::time::Duration::from_millis(5));
            return handshake_loop(tls_stream, timeout);
        }
        Err(HandshakeError::Failure(_)) => panic!("OMG"),
    };
}

#[cfg(feature = "use-native-tls")]
fn handshake_loop(
    tls_stream: MidHandshakeTlsStream<TcpStream>,
    timeout: Duration,
) -> Result<TlsStream<TcpStream>, std::io::Error> {
    let now = Instant::now();
    let mut tls_stream = tls_stream;
    loop {
        let result = tls_stream.handshake();
        match result {
            Ok(connected_stream) => {
                debug!("TLS connection established!");
                return Ok(connected_stream);
            }
            Err(HandshakeError::WouldBlock(next_stream)) => {
                if now.elapsed() >= timeout {
                    return Err(std::io::ErrorKind::TimedOut.into());
                }
                trace!("Socket is not ready, backing off for a bit...");
                std::thread::sleep(std::time::Duration::from_millis(5));
                tls_stream = next_stream;
            }
            Err(HandshakeError::Failure(_)) => {
                // debug!("failure!");
                panic!("OMG");
            }
        };
    }
}

impl Read for IoStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.try_read_into_buffer(buf)
    }
}

pub trait NonblockingSocket {
    fn send(&mut self, buf: &[u8]) -> Result<(), std::io::Error>;
    fn try_send(&mut self, buf: &[u8]) -> Result<(), std::io::Error>;
    fn read_blocking(&mut self) -> Result<Vec<u8>, std::io::Error>;
    fn try_read(&mut self) -> Result<Option<Vec<u8>>, std::io::Error>;
    fn try_read_into_buffer(&mut self, buffer: &mut [u8]) -> Result<usize, std::io::Error>;
}

impl NonblockingSocket for IoStream {
    fn send(&mut self, buf: &[u8]) -> Result<(), std::io::Error> {
        loop {
            let write_res = self.stream.write_all(&buf[..]);
            match write_res {
                Ok(_) => return Result::Ok(()),
                Err(x) => match x.kind() {
                    ErrorKind::Interrupted => {}
                    ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(5)),
                    ErrorKind::NotConnected => return Result::Err(x),
                    _ => return Result::Err(x),
                },
            }
        }
    }

    fn try_send(&mut self, buf: &[u8]) -> Result<(), std::io::Error> {
        loop {
            let write_res = self.stream.write_all(&buf[..]);
            match write_res {
                Ok(_) => return Result::Ok(()),
                Err(x) => match x.kind() {
                    ErrorKind::Interrupted => {}
                    other_code => return Result::Err(std::io::Error::from(other_code)),
                },
            }
        }
    }

    fn read_blocking(&mut self) -> Result<Vec<u8>, std::io::Error> {
        loop {
            let len = 1024 * 1024;
            let mut res = vec![0; len];
            let read_res = self.stream.read(&mut res);
            match read_res {
                Ok(_) => return Ok(res),
                Err(x) => match x.kind() {
                    ErrorKind::Interrupted => {}
                    ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(5)),
                    kind => {
                        debug!("read error. Raw OS Error: {:?}, Kind: {:?}, Source: {:?} Description: {:?}", 
                            x.raw_os_error(), kind, x.source(), x);
                        return Result::Err(x);
                    }
                },
            }
        }
    }

    fn try_read_into_buffer(&mut self, buffer: &mut [u8]) -> Result<usize, std::io::Error> {
        loop {
            let read_res = self.stream.read(buffer);
            match read_res {
                Ok(0) => return Err(ErrorKind::ConnectionReset.into()),
                Ok(length) => {
                    return Ok(length);
                }
                Err(x) => match x.kind() {
                    ErrorKind::Interrupted => {}
                    ErrorKind::WouldBlock => return Ok(0),
                    kind => {
                        debug!("read error. Raw OS Error: {:?}, Kind: {:?}, Source: {:?} Description: {:?}", 
                                                    x.raw_os_error(), kind, x.source(), x);
                        return Err(x);
                    }
                },
            }
        }
    }

    fn try_read(&mut self) -> Result<Option<Vec<u8>>, std::io::Error> {
        let len = 1024 * 1024;
        let mut res = vec![0; len];
        loop {
            let read_res = self.stream.read(&mut res);
            match read_res {
                Ok(0) => return Err(ErrorKind::ConnectionReset.into()),
                Ok(_length) => {
                    return Ok(Some(res[0.._length].to_vec()));
                }
                Err(x) => match x.kind() {
                    ErrorKind::Interrupted => {}
                    ErrorKind::WouldBlock => return Ok(None),
                    kind => {
                        debug!("read error. Raw OS Error: {:?}, Kind: {:?}, Source: {:?} Description: {:?}", 
                                                    x.raw_os_error(), kind, x.source(), x);
                        return Err(x);
                    }
                },
            }
        }
    }
}
