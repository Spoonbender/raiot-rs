use std::sync::mpsc::{Receiver, Sender};
use std::{io::ErrorKind, io::Read, io::Write, sync::mpsc};

use mpsc::TryRecvError;
use raiot_buffers::CircularBuffer;

pub struct MockSocket {}

pub struct MockClientSocket {
    write_data_tx: Sender<Vec<u8>>,
    read_data_rx: Receiver<Vec<u8>>,
    read_ctl_rx: Receiver<std::io::Result<usize>>,
    write_ctl_rx: Receiver<std::io::Result<usize>>,
    read_data_buf: CircularBuffer,
}

impl MockClientSocket {
    fn read_from_buffer(&mut self, size: usize, buf: &mut [u8]) {
        while self.read_data_buf.valid_length() < size {
            match self.read_data_rx.try_recv() {
                Ok(next_bytes) => {
                    self.read_data_buf.write_all(&next_bytes).unwrap();
                }
                Err(e) => match e {
                    TryRecvError::Empty => {
                        break;
                    }
                    _other => panic!("ODMFODF"),
                },
            }
        }

        let size = std::cmp::min(self.read_data_buf.valid_length(), size);
        let mut res = self.read_data_buf.read_bytes(size);
        let subbuf = &mut buf[0..size];
        res.read_exact(subbuf).unwrap();
    }
}

impl Write for MockClientSocket {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let res = self.write_ctl_rx.try_recv();
        match res {
            Err(TryRecvError::Empty) => {
                return Err(ErrorKind::WouldBlock.into());
            }
            Err(TryRecvError::Disconnected) => {
                panic!("Discooo");
            }
            Ok(Ok(usize)) => {
                let send_size = std::cmp::min(buf.len(), usize);
                let send_vec = buf[0..send_size].into();
                self.write_data_tx.send(send_vec).unwrap();
                Ok(send_size)
            }
            Ok(Err(e)) => Err(e),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Read for MockClientSocket {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.read_ctl_rx.try_recv() {
            Ok(res) => match res {
                Ok(size) => {
                    let read_size = std::cmp::min(buf.len(), size);
                    self.read_from_buffer(read_size, buf);
                    return Ok(read_size);
                }
                Err(e) => {
                    return Err(e);
                }
            },
            Err(TryRecvError::Empty) => {
                return Err(ErrorKind::WouldBlock.into());
            }
            Err(TryRecvError::Disconnected) => {
                panic!("Discooo");
            }
        }
    }
}

pub struct MockServerSocket {
    write_data_tx: Sender<Vec<u8>>,
    read_data_rx: Receiver<Vec<u8>>,
    read_ctl_tx: Sender<std::io::Result<usize>>,
    write_ctl_tx: Sender<std::io::Result<usize>>,
    read_data_buf: CircularBuffer,
}

impl MockServerSocket {
    pub fn push_read_ctl(&mut self, ctl: std::io::Result<usize>) {
        self.read_ctl_tx.send(ctl).unwrap();
    }

    pub fn push_write_ctl(&mut self, ctl: std::io::Result<usize>) {
        self.write_ctl_tx.send(ctl).unwrap();
    }

    pub fn push_data(&mut self, buf: &[u8]) {
        self.write_data_tx.send(buf.into()).unwrap();
    }

    fn read_from_buffer(&mut self, buf: &mut [u8]) -> usize {
        while self.read_data_buf.valid_length() < buf.len() {
            match self.read_data_rx.try_recv() {
                Ok(next_bytes) => {
                    self.read_data_buf.write_all(&next_bytes).unwrap();
                }
                Err(TryRecvError::Empty) => {
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    panic!("disco");
                }
            }
        }

        let read_size = std::cmp::min(buf.len(), self.read_data_buf.valid_length());
        let mut res = self.read_data_buf.read_bytes(read_size);
        res.read_exact(buf).unwrap();
        return read_size;
    }
}

impl Read for MockServerSocket {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        return Ok(self.read_from_buffer(buf));
    }
}

impl MockSocket {
    pub fn create() -> (MockClientSocket, MockServerSocket) {
        // channel for server-to-client data
        let (server_data_tx, client_data_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
            mpsc::channel();
        // channel for client-to-server data
        let (client_data_tx, server_data_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
            mpsc::channel();
        // channel for server-to-client control
        let (write_ctl_tx, write_ctl_rx): (
            Sender<std::io::Result<usize>>,
            Receiver<std::io::Result<usize>>,
        ) = mpsc::channel();
        // channel for client-to-server control
        let (read_ctl_tx, read_ctl_rx): (
            Sender<std::io::Result<usize>>,
            Receiver<std::io::Result<usize>>,
        ) = mpsc::channel();
        let client_read_data_buf = CircularBuffer::new(1024 * 1024);
        let server_read_data_buf = CircularBuffer::new(1024 * 1024);

        let client = MockClientSocket {
            write_data_tx: client_data_tx,
            read_data_rx: client_data_rx,
            write_ctl_rx,
            read_ctl_rx,
            read_data_buf: client_read_data_buf,
        };

        let server = MockServerSocket {
            write_data_tx: server_data_tx,
            read_data_rx: server_data_rx,
            read_ctl_tx,
            write_ctl_tx,
            read_data_buf: server_read_data_buf,
        };

        return (client, server);
    }
}
