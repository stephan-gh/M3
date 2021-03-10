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

use m3::cell::{Ref, RefCell};
use m3::com::RecvGate;
use m3::errors::{Code, Error};
use m3::net::{IpAddr, NetData, SocketState, SocketType, UdpState};
use m3::rc::Rc;
use m3::log;

use smoltcp;
use smoltcp::socket::SocketSet;
use smoltcp::socket::{RawSocket, SocketHandle, TcpSocket, UdpSocket};
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

use crate::sess::FileSession;

//Needed to create correct buffer sizes
pub const TCP_HEADER_SIZE: usize = 32;
pub const UDP_HEADER_SIZE: usize = 8;


///Allows us to convert a smol tcp state to a m3 tcp state. Cannot use the From trait since we would implement on a foreign type.
fn tcp_state_from_smoltcp_state(other: smoltcp::socket::TcpState) -> m3::net::TcpState {
    match other {
        smoltcp::socket::TcpState::Closed => m3::net::TcpState::Closed,
        smoltcp::socket::TcpState::Listen => m3::net::TcpState::Listen,
        smoltcp::socket::TcpState::SynSent => m3::net::TcpState::SynSent,
        smoltcp::socket::TcpState::SynReceived => m3::net::TcpState::SynReceived,
        smoltcp::socket::TcpState::Established => m3::net::TcpState::Established,
        smoltcp::socket::TcpState::FinWait1 => m3::net::TcpState::FinWait1,
        smoltcp::socket::TcpState::FinWait2 => m3::net::TcpState::FinWait2,
        smoltcp::socket::TcpState::CloseWait => m3::net::TcpState::CloseWait,
        smoltcp::socket::TcpState::Closing => m3::net::TcpState::Closing,
        smoltcp::socket::TcpState::LastAck => m3::net::TcpState::LastAck,
        smoltcp::socket::TcpState::TimeWait => m3::net::TcpState::TimeWait,
    }
}

///Socket abstraction
pub struct Socket {
    pub sd: i32,
    //The handle into the global socket set, used to get the smol socket.
    pub socket: SocketHandle,
    //tracks the internal type
    pub ty: SocketType,

    pub socket_session_rgate: Rc<RefCell<RecvGate>>,
    pub rgate: Option<Rc<RefCell<RecvGate>>>,
    //Might be a file session
    pub rfile: Option<Rc<RefCell<FileSession>>>,
    pub sfile: Option<Rc<RefCell<FileSession>>>,
}

impl Socket {
    pub fn from_smol_socket(
        socket: SocketHandle,
        ty: SocketType,
        socket_session_rgate: Rc<RefCell<RecvGate>>,
    ) -> Self {
        Socket {
            sd: -1, //Invalid socket for now
            socket,
            ty,

            socket_session_rgate,
            rgate: None,

            rfile: None,
            sfile: None,
        }
    }

    ///returns a reference to the parents socket session's rgate
    pub fn socket_session_rgate<'a>(&'a self) -> Ref<'a, RecvGate> {
        self.socket_session_rgate.borrow()
    }

    pub fn bind(
        &mut self,
        endpoint: IpEndpoint,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Dgram {
            log!(crate::LOG_DEF, "Can not bind tcp or raw socket!");
            return Err(Error::new(Code::NoSpace));
        }
        //If Udp socket, bind, otherwise do nothing, since in smoltcp, the tcp_bind event is fused with tcp_listen.
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
            Ok(())
        }
    }

    ///Tries to receive a package on this socket. Depending on the type of this socket, the data might be a raw
    /// ethernet frame (raw sockets) or some byte data (tcp/udp sockets).
    /// Returns (remote_endpoint, data)
    pub fn receive<'a>(
        &mut self,
        socket_set: &'a mut SocketSet<'static>,
    ) -> Result<NetData, Error> {
        //Currently allocating Vec<u8> since the result is going into a NetData package for marshalling anyways.
        //However it would be possible to use slices here if the marshalled package would use a slice.
        match self.ty {
            SocketType::Stream => {
                let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                let addr = tcp_socket.remote_endpoint();
                match tcp_socket.recv(|d| (d.len(), d)) {
                    Ok(buf) => {
                        //Build the net data struct
                        let (m3addr, port) = crate::util::to_m3_addr(addr);
                        let data =
                            NetData::from_slice(0, buf, m3addr, port, IpAddr::unspecified(), 0); //sd gets set in the socket session

                        Ok(data)
                    },
                    Err(_e) => Err(Error::new(Code::NoSpace)),
                }
            },
            SocketType::Dgram => {
                let mut udp_socket = socket_set.get::<UdpSocket>(self.socket);
                match udp_socket.recv() {
                    Ok((data, remote_endpoint)) => {
                        let (m3addr, port) = crate::util::to_m3_addr(remote_endpoint);
                        let data =
                            NetData::from_slice(0, data, m3addr, port, IpAddr::unspecified(), 0); //sd gets set in the socket session

                        Ok(data)
                    },
                    Err(_e) => Err(Error::new(Code::NoSpace)),
                }
            },
            SocketType::Raw => {
                let mut raw_socket = socket_set.get::<RawSocket>(self.socket);
                match raw_socket.recv() {
                    Ok(data) => {
                        let (m3addr, port) = crate::util::to_m3_addr(IpEndpoint::UNSPECIFIED);
                        let data =
                            NetData::from_slice(0, data, m3addr, port, IpAddr::unspecified(), 0); //sd gets set in the socket session

                        Ok(data)
                    },
                    Err(_e) => Err(Error::new(Code::NoSpace)),
                }
            },
            SocketType::Undefined => Err(Error::new(Code::NoSuchSocket)),
        }
    }

    pub fn close(&mut self, socket_set: &mut SocketSet<'static>) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            log!(crate::LOG_DEF, "Udp or raw socket can't be closed!");
            return Err(Error::new(Code::NoSpace));
        }

        let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
        tcp_socket.close();
        Ok(())
    }

    ///Send data over this socket connect, if everything is set alright. Returns the smoltcp error if something failed.
    pub fn send_data_slice(
        &mut self,
        data: NetData,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<usize, smoltcp::Error> {
        let size = data.data.len();
        log!(crate::LOG_DEF, "send_data: size={}", size);

        let res = match self.ty {
            SocketType::Stream => {
                let mut tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                log!(
                    crate::LOG_DEF,
                    "TCP: Send: src={}, dst={}, data_size={}",
                    tcp_socket.local_endpoint(),
                    tcp_socket.remote_endpoint(),
                    data.size
                );
                tcp_socket.send_slice(data.raw_data())
            },
            SocketType::Dgram => {
                //on udp send dictates the destination
                let rend = endpoint(data.dest_addr, data.dest_port);
                log!(
                    crate::LOG_DEF,
                    "UDP: Send: dst={}, data_size={}",
                    rend,
                    data.size
                );
                let mut udp_socket = socket_set.get::<UdpSocket>(self.socket);
                match udp_socket.send_slice(data.raw_data(), rend) {
                    Ok(_) => Ok(size),
                    Err(e) => Err(e),
                }
            },
            SocketType::Raw => {
                let mut raw_socket = socket_set.get::<RawSocket>(self.socket);
                log!(crate::LOG_DEF, "RAW: Send: data_size={}", data.size);
                match raw_socket.send_slice(data.raw_data()) {
                    Ok(_) => Ok(size),
                    Err(e) => Err(e),
                }
            },
            SocketType::Undefined => {
                log!(crate::LOG_DEF, "Can't send on undefined socket!");
                Ok(0)
            },
        };

        res
    }

    ///returns the socket state depending on the socket type
    pub fn get_state(&self, socket_set: &mut SocketSet<'static>) -> Result<SocketState, Error> {
        match self.ty {
            SocketType::Stream => {
                let tcp_socket = socket_set.get::<TcpSocket>(self.socket);
                Ok(SocketState::TcpState(tcp_state_from_smoltcp_state(
                    tcp_socket.state(),
                )))
            },
            SocketType::Dgram => {
                //udp socket can only be bound or unbound, therefore just check if send is true
                let udp_socket = socket_set.get::<UdpSocket>(self.socket);
                let udp_state = if udp_socket.is_open() {
                    UdpState::Open
                }
                else {
                    UdpState::Unbound
                };

                Ok(SocketState::UdpState(udp_state))
            },
            SocketType::Raw => Ok(SocketState::RawState),
            SocketType::Undefined => Err(Error::new(Code::NoSuchSocket)),
        }
    }
}

fn endpoint(addr: IpAddr, port: u16) -> IpEndpoint {
    IpEndpoint::new(
        IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.0.to_be_bytes())),
        port,
    )
}
