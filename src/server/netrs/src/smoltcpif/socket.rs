/*
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
use m3::net::{event, IpAddr, Port, Sd, SocketType};
use m3::rc::Rc;

use smoltcp;
use smoltcp::socket::SocketSet;
use smoltcp::socket::{RawSocket, SocketHandle, TcpSocket, TcpState, UdpSocket};
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

use crate::sess::FileSession;

// Needed to create correct buffer sizes
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

/// Socket abstraction
pub struct Socket {
    sd: Sd,
    // The handle into the global socket set, used to get the smol socket.
    socket: SocketHandle,
    // tracks the internal type
    ty: SocketType,
    state: State,

    // Might be a file session
    rfile: Option<Rc<RefCell<FileSession>>>,
    sfile: Option<Rc<RefCell<FileSession>>>,
}

impl Socket {
    pub fn new(sd: Sd, socket: SocketHandle, ty: SocketType) -> Self {
        Socket {
            sd,
            socket,
            ty,
            state: State::None,

            rfile: None,
            sfile: None,
        }
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
        endpoint: IpEndpoint,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Dgram {
            log!(crate::LOG_DEF, "Can not bind tcp or raw socket!");
            return Err(Error::new(Code::WrongSocketType));
        }

        // If Udp socket, bind, otherwise do nothing, since in smoltcp, the tcp_bind event is fused with tcp_listen.
        let mut udp_socket = socket_set.get::<UdpSocket>(self.socket);
        log!(crate::LOG_DEF, "Binding Udp socket: {}", endpoint);
        if let Err(e) = udp_socket.bind(endpoint) {
            log!(crate::LOG_DEF, "Udp::bind() failed with: {}", e);
            Err(Error::new(Code::BindFailed))
        }
        else {
            Ok(())
        }
    }

    pub fn listen(
        &mut self,
        socket_set: &mut SocketSet<'static>,
        local_endpoint: IpEndpoint,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            log!(crate::LOG_DEF, "Can not listen on udp or raw socket!");
            return Err(Error::new(Code::WrongSocketType));
        }

        let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
        log!(
            crate::LOG_DEF,
            "Listening on TCP socket: {}",
            local_endpoint
        );

        if let Err(e) = tcp_socket.listen(local_endpoint) {
            log!(crate::LOG_DEF, "Tcp::listen() failed with: {}", e);
            Err(Error::new(Code::ListenFailed))
        }
        else {
            self.state = State::Connecting;
            Ok(())
        }
    }

    pub fn connect(
        &mut self,
        remote_endpoint: IpEndpoint,
        local_endpoint: IpEndpoint,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            log!(crate::LOG_DEF, "Udp or raw socket can't be connected!");
            return Err(Error::new(Code::WrongSocketType));
        }

        let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
        if let Err(e) = tcp_socket.connect(remote_endpoint, local_endpoint) {
            log!(crate::LOG_DEF, "Failed to connect socket: {}", e);
            Err(Error::new(Code::ConnectionFailed))
        }
        else {
            self.state = State::Connecting;
            Ok(())
        }
    }

    /// Tries to receive a package on this socket. Depending on the type of this socket, the data might be a raw
    /// ethernet frame (raw sockets) or some byte data (tcp/udp sockets).
    /// Returns (remote_endpoint, data)
    pub fn receive<F>(
        &mut self,
        socket_set: &mut SocketSet<'static>,
        func: F,
    ) -> Result<(), smoltcp::Error>
    where
        F: FnOnce(&[u8], IpEndpoint) -> usize,
    {
        match self.ty {
            SocketType::Stream => {
                let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                if self.state == State::Connected {
                    let addr = tcp_socket.remote_endpoint();
                    tcp_socket.recv(|d| {
                        if d.len() > 0 {
                            (func(d, addr), ())
                        }
                        else {
                            (0, ())
                        }
                    })
                }
                else {
                    Ok(())
                }
            },
            SocketType::Dgram => {
                let mut udp_socket = socket_set.get::<UdpSocket>(self.socket);
                match udp_socket.recv() {
                    Ok((data, remote_endpoint)) => {
                        func(data, remote_endpoint);
                        Ok(())
                    },
                    Err(e) => Err(e),
                }
            },
            SocketType::Raw => {
                let mut raw_socket = socket_set.get::<RawSocket>(self.socket);
                match raw_socket.recv() {
                    Ok(data) => {
                        func(data, IpEndpoint::UNSPECIFIED);
                        Ok(())
                    },
                    Err(e) => Err(e),
                }
            },
            // TODO fix error
            SocketType::Undefined => Err(smoltcp::Error::Unrecognized),
        }
    }

    pub fn close(&mut self, socket_set: &mut SocketSet<'static>) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            log!(crate::LOG_DEF, "Udp or raw socket can't be closed!");
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

    pub fn send_data_slice(
        &mut self,
        data: &[u8],
        dest_addr: IpAddr,
        dest_port: u16,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self.ty {
            SocketType::Stream => {
                let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                if !tcp_socket.can_send() {
                    return Err(Error::new(Code::NoSpace));
                }

                log!(
                    crate::LOG_DEF,
                    "TCP: Send: src={}, dst={}, data_size={}",
                    tcp_socket.local_endpoint(),
                    tcp_socket.remote_endpoint(),
                    data.len() as usize
                );

                tcp_socket.send_slice(data).unwrap();
                Ok(())
            },

            SocketType::Dgram => {
                let mut udp_socket = socket_set.get::<UdpSocket>(self.socket);
                if !udp_socket.can_send() {
                    return Err(Error::new(Code::NoSpace));
                }

                // on udp send dictates the destination
                let rend = IpEndpoint::new(
                    IpAddress::Ipv4(Ipv4Address::from_bytes(&dest_addr.0.to_be_bytes())),
                    dest_port,
                );

                log!(
                    crate::LOG_DEF,
                    "UDP: Send: dst={}, data_size={} (capacity={}, bytes={})",
                    rend,
                    data.len() as usize,
                    udp_socket.packet_send_capacity(),
                    udp_socket.payload_send_capacity(),
                );

                udp_socket.send_slice(data, rend).unwrap();
                Ok(())
            },

            SocketType::Raw => {
                let mut raw_socket = socket_set.get::<RawSocket>(self.socket);
                if !raw_socket.can_send() {
                    return Err(Error::new(Code::NoSpace));
                }

                log!(
                    crate::LOG_DEF,
                    "RAW: Send: data_size={}",
                    data.len() as usize
                );

                raw_socket.send_slice(data).unwrap();
                Ok(())
            },

            SocketType::Undefined => {
                log!(crate::LOG_DEF, "Can't send on undefined socket!");
                Err(Error::new(Code::NotSup))
            },
        }
    }
}
