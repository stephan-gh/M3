/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use m3::cell::RefCell;
use m3::errors::{Code, Error};
use m3::log;
use m3::net::{event, IpAddr, Port, Sd, SocketType, MAX_NETDATA_SIZE, MSG_BUF_SIZE};
use m3::rc::Rc;
use m3::vec;

use smoltcp;
use smoltcp::socket::SocketSet;
use smoltcp::socket::{
    RawSocket, RawSocketBuffer, SocketHandle, TcpSocket, TcpSocketBuffer, TcpState, UdpSocket,
    UdpSocketBuffer,
};
use smoltcp::storage::PacketMetadata;
use smoltcp::wire::IpVersion;
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

use crate::sess::FileSession;

pub const MAX_SEND_BUF_PACKETS: usize = 8;
pub const MAX_RECV_BUF_PACKETS: usize = 32;

/// Defines how big the socket buffers must be, currently this is the max NetDataSize multiplied by the
/// Maximum in flight packages
pub const TCP_BUFFER_SIZE: usize = (MAX_NETDATA_SIZE + TCP_HEADER_SIZE) * MAX_RECV_BUF_PACKETS;
pub const UDP_BUFFER_SIZE: usize = (MAX_NETDATA_SIZE + UDP_HEADER_SIZE) * MAX_RECV_BUF_PACKETS;
pub const RAW_BUFFER_SIZE: usize = MAX_NETDATA_SIZE * MAX_RECV_BUF_PACKETS;

pub const TCP_HEADER_SIZE: usize = 32;
pub const UDP_HEADER_SIZE: usize = 8;

/// Converts an IpEndpoint from smoltcp into an M³ (IpAddr, Port) tuple.
/// Assumes that the IpEndpoint a is Ipv4 address, otherwise this will panic.
pub fn to_m3_addr(addr: IpEndpoint) -> (IpAddr, Port) {
    assert!(addr.addr.as_bytes().len() == 4, "Address was not ipv4!");
    let bytes = addr.addr.as_bytes();
    (
        IpAddr::new(bytes[0], bytes[1], bytes[2], bytes[3]),
        addr.port,
    )
}

#[derive(Debug)]
pub enum SendNetEvent {
    Connected(event::ConnectedMessage),
    Closed(event::ClosedMessage),
    CloseReq(event::CloseReqMessage),
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum State {
    None,
    Connecting,
    Connected,
}

/// Socket abstraction that unifies the different socket types
pub struct Socket {
    sd: Sd,
    socket: SocketHandle,
    ty: SocketType,
    state: State,

    // for the file session
    rfile: Option<Rc<RefCell<FileSession>>>,
    sfile: Option<Rc<RefCell<FileSession>>>,
}

impl Socket {
    pub fn new(
        sd: Sd,
        ty: SocketType,
        protocol: u8,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<Self, Error> {
        let socket = match ty {
            SocketType::Stream => socket_set.add(TcpSocket::new(
                TcpSocketBuffer::new(vec![0 as u8; TCP_BUFFER_SIZE]),
                TcpSocketBuffer::new(vec![0 as u8; TCP_BUFFER_SIZE]),
            )),
            SocketType::Dgram => socket_set.add(UdpSocket::new(
                UdpSocketBuffer::new(
                    vec![PacketMetadata::EMPTY; MAX_RECV_BUF_PACKETS],
                    vec![0 as u8; UDP_BUFFER_SIZE],
                ),
                UdpSocketBuffer::new(
                    vec![PacketMetadata::EMPTY; MAX_SEND_BUF_PACKETS],
                    vec![0 as u8; UDP_BUFFER_SIZE],
                ),
            )),
            SocketType::Raw => socket_set.add(RawSocket::new(
                IpVersion::Ipv4,
                protocol.into(),
                RawSocketBuffer::new(
                    vec![PacketMetadata::EMPTY; MSG_BUF_SIZE],
                    vec![0 as u8; RAW_BUFFER_SIZE],
                ),
                RawSocketBuffer::new(
                    vec![PacketMetadata::EMPTY; MSG_BUF_SIZE],
                    vec![0 as u8; RAW_BUFFER_SIZE],
                ),
            )),
            _ => return Err(Error::new(Code::InvArgs)),
        };

        Ok(Socket {
            sd,
            socket,
            ty,
            state: State::None,

            rfile: None,
            sfile: None,
        })
    }

    pub fn sd(&self) -> Sd {
        self.sd
    }

    pub fn recv_file(&self) -> Option<&Rc<RefCell<FileSession>>> {
        self.rfile.as_ref()
    }

    pub fn send_file(&self) -> Option<&Rc<RefCell<FileSession>>> {
        self.sfile.as_ref()
    }

    pub fn set_recv_file(&mut self, file: Option<Rc<RefCell<FileSession>>>) {
        self.rfile = file;
    }

    pub fn set_send_file(&mut self, file: Option<Rc<RefCell<FileSession>>>) {
        self.sfile = file;
    }

    pub fn fetch_event(&mut self, socket_set: &mut SocketSet<'static>) -> Option<SendNetEvent> {
        match (self.ty, self.state) {
            (SocketType::Stream, State::Connecting) => {
                let tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                if tcp_socket.state() == TcpState::Established {
                    self.state = State::Connected;
                    let (ip, port) = to_m3_addr(tcp_socket.remote_endpoint());
                    Some(SendNetEvent::Connected(event::ConnectedMessage::new(
                        self.sd, ip, port,
                    )))
                }
                else {
                    None
                }
            },

            (SocketType::Stream, State::Connected) => {
                let tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                if !tcp_socket.is_open() {
                    self.state = State::None;
                    Some(SendNetEvent::Closed(event::ClosedMessage::new(self.sd)))
                }
                // remote side has closed the connection?
                else if tcp_socket.state() == TcpState::CloseWait {
                    Some(SendNetEvent::CloseReq(event::CloseReqMessage::new(self.sd)))
                }
                else {
                    None
                }
            },

            _ => None,
        }
    }

    pub fn bind(
        &mut self,
        addr: IpAddr,
        port: Port,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Dgram {
            return Err(Error::new(Code::WrongSocketType));
        }

        let endpoint = IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.0.to_be_bytes())),
            port,
        );

        let mut udp_socket = socket_set.get::<UdpSocket>(self.socket);
        udp_socket.bind(endpoint).map_err(|e| {
            log!(crate::LOG_ERR, "bind failed: {}", e);
            Error::new(Code::BindFailed)
        })
    }

    pub fn listen(
        &mut self,
        socket_set: &mut SocketSet<'static>,
        addr: IpAddr,
        port: Port,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            return Err(Error::new(Code::WrongSocketType));
        }

        let endpoint = IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.0.to_be_bytes())),
            port,
        );

        let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
        match tcp_socket.listen(endpoint) {
            Ok(_) => {
                self.state = State::Connecting;
                Ok(())
            },
            Err(e) => {
                log!(crate::LOG_ERR, "listen failed: {}", e);
                Err(Error::new(Code::ListenFailed))
            },
        }
    }

    pub fn connect(
        &mut self,
        remote_addr: IpAddr,
        remote_port: Port,
        local_port: Port,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            return Err(Error::new(Code::WrongSocketType));
        }

        let remote_endpoint = IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address::from_bytes(&remote_addr.0.to_be_bytes())),
            remote_port,
        );
        let local_endpoint = IpEndpoint::from(local_port);

        let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
        match tcp_socket.connect(remote_endpoint, local_endpoint) {
            Ok(_) => {
                self.state = State::Connecting;
                Ok(())
            },
            Err(e) => {
                log!(crate::LOG_ERR, "connect failed: {}", e);
                Err(Error::new(Code::ConnectionFailed))
            },
        }
    }

    pub fn close(&mut self, socket_set: &mut SocketSet<'static>) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            return Err(Error::new(Code::WrongSocketType));
        }

        let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
        tcp_socket.close();
        Ok(())
    }

    pub fn abort(&mut self, socket_set: &mut SocketSet<'static>) {
        if self.ty == SocketType::Stream {
            let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
            tcp_socket.abort();
        }

        self.state = State::None;
    }

    pub fn receive<F>(&mut self, socket_set: &mut SocketSet<'static>, func: F)
    where
        F: FnOnce(&[u8], IpEndpoint) -> usize,
    {
        match self.ty {
            SocketType::Stream => {
                let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                if self.state == State::Connected {
                    let addr = tcp_socket.remote_endpoint();
                    // don't even log errors here, since they occur often and are uninteresting
                    tcp_socket
                        .recv(|d| {
                            if d.len() > 0 {
                                (func(d, addr), ())
                            }
                            else {
                                (0, ())
                            }
                        })
                        .ok();
                }
            },

            SocketType::Dgram => {
                let mut udp_socket = socket_set.get::<UdpSocket>(self.socket);
                if let Ok((data, remote_endpoint)) = udp_socket.recv() {
                    func(data, remote_endpoint);
                }
            },

            SocketType::Raw => {
                let mut raw_socket = socket_set.get::<RawSocket>(self.socket);
                if let Ok(data) = raw_socket.recv() {
                    func(data, IpEndpoint::UNSPECIFIED);
                }
            },

            SocketType::Undefined => panic!("cannot receive from undefined socket"),
        }
    }

    pub fn send(
        &mut self,
        data: &[u8],
        dest_addr: IpAddr,
        dest_port: Port,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self.ty {
            SocketType::Stream => {
                let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                if !tcp_socket.can_send() {
                    return Err(Error::new(Code::NoSpace));
                }

                tcp_socket.send_slice(data).unwrap();
                Ok(())
            },

            SocketType::Dgram => {
                let mut udp_socket = socket_set.get::<UdpSocket>(self.socket);
                if !udp_socket.can_send() {
                    return Err(Error::new(Code::NoSpace));
                }

                let rend = IpEndpoint::new(
                    IpAddress::Ipv4(Ipv4Address::from_bytes(&dest_addr.0.to_be_bytes())),
                    dest_port,
                );

                udp_socket.send_slice(data, rend).unwrap();
                Ok(())
            },

            SocketType::Raw => {
                let mut raw_socket = socket_set.get::<RawSocket>(self.socket);
                if !raw_socket.can_send() {
                    return Err(Error::new(Code::NoSpace));
                }

                raw_socket.send_slice(data).unwrap();
                Ok(())
            },

            SocketType::Undefined => panic!("cannot send to undefined socket"),
        }
    }
}