/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

use m3::cap::Selector;
use m3::cell::RefCell;
use m3::errors::{Code, Error};
use m3::log;
use m3::mem::size_of;
use m3::net::{
    event, DataQueue, Endpoint, IpAddr, NetEvent, NetEventChannel, NetEventType, Port, Sd,
    SocketArgs, SocketType,
};
use m3::rc::Rc;
use m3::time::{TimeDuration, TimeInstant};
use m3::vec;

use smoltcp::socket::SocketSet;
use smoltcp::socket::{
    RawSocket, RawSocketBuffer, SocketHandle, TcpSocket, TcpSocketBuffer, TcpState, UdpSocket,
    UdpSocketBuffer,
};
use smoltcp::storage::PacketMetadata;
use smoltcp::wire::IpVersion;
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};

use crate::ports::{AnyPort, EphemeralPort};
use crate::sess::FileSession;

const CONNECT_TIMEOUT: TimeDuration = TimeDuration::from_secs(6);

pub fn to_m3_addr(addr: IpAddress) -> IpAddr {
    if addr.as_bytes().len() != 4 {
        IpAddr::unspecified()
    }
    else {
        let bytes = addr.as_bytes();
        IpAddr::new(bytes[0], bytes[1], bytes[2], bytes[3])
    }
}

/// Converts an IpEndpoint from smoltcp into an M³ (IpAddr, Port) tuple.
/// Assumes that the IpEndpoint a is Ipv4 address, otherwise this will panic.
pub fn to_m3_ep(addr: IpEndpoint) -> Endpoint {
    Endpoint::new(to_m3_addr(addr.addr), addr.port)
}

#[derive(Debug)]
pub enum SendNetEvent {
    Connected(event::ConnectedMessage),
    Closed(event::ClosedMessage),
    CloseReq(event::CloseReqMessage),
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum State {
    Closed,
    Bound,
    Connecting,
    Connected,
}

/// Socket abstraction that unifies the different socket types
pub struct Socket {
    sd: Sd,
    socket: SocketHandle,
    ty: SocketType,
    state: State,
    connect_start: Option<TimeInstant>,
    _local_port: Option<EphemeralPort>,
    buffer_space: usize,

    // communication channel to client for incoming data/close-requests and outgoing events/data
    channel: Rc<NetEventChannel>,
    // pending incoming data events we could not send due to missing buffer space
    send_queue: DataQueue,

    // for the file session
    rfile: Option<Rc<RefCell<FileSession>>>,
    sfile: Option<Rc<RefCell<FileSession>>>,
}

impl Socket {
    pub fn required_space(ty: SocketType, args: &SocketArgs) -> usize {
        args.rbuf_size
            + args.sbuf_size
            + match ty {
                SocketType::Dgram => {
                    (args.sbuf_slots + args.rbuf_slots) * size_of::<UdpSocketBuffer<'_>>()
                },
                SocketType::Raw => {
                    (args.sbuf_slots + args.rbuf_slots) * size_of::<RawSocketBuffer<'_>>()
                },
                _ => 0,
            }
    }

    pub fn new(
        sd: Sd,
        ty: SocketType,
        protocol: u8,
        args: &SocketArgs,
        caps: Selector,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<Self, Error> {
        let socket = match ty {
            SocketType::Stream => socket_set.add(TcpSocket::new(
                TcpSocketBuffer::new(vec![0u8; args.rbuf_size]),
                TcpSocketBuffer::new(vec![0u8; args.sbuf_size]),
            )),
            SocketType::Dgram => socket_set.add(UdpSocket::new(
                UdpSocketBuffer::new(vec![PacketMetadata::EMPTY; args.rbuf_slots], vec![
                    0u8;
                    args.rbuf_size
                ]),
                UdpSocketBuffer::new(vec![PacketMetadata::EMPTY; args.sbuf_slots], vec![
                    0u8;
                    args.sbuf_size
                ]),
            )),
            SocketType::Raw => socket_set.add(RawSocket::new(
                IpVersion::Ipv4,
                protocol.into(),
                RawSocketBuffer::new(vec![PacketMetadata::EMPTY; args.rbuf_slots], vec![
                    0u8;
                    args.rbuf_size
                ]),
                RawSocketBuffer::new(vec![PacketMetadata::EMPTY; args.sbuf_slots], vec![
                    0u8;
                    args.sbuf_size
                ]),
            )),
            _ => return Err(Error::new(Code::InvArgs)),
        };

        Ok(Socket {
            sd,
            socket,
            ty,
            state: State::Closed,
            connect_start: None,
            _local_port: None,
            buffer_space: Self::required_space(ty, args),

            channel: NetEventChannel::new_server(caps)?,
            send_queue: DataQueue::default(),

            rfile: None,
            sfile: None,
        })
    }

    pub fn sd(&self) -> Sd {
        self.sd
    }

    pub fn channel(&self) -> &Rc<NetEventChannel> {
        &self.channel
    }

    pub fn buffer_space(&self) -> usize {
        self.buffer_space
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
                let mut tcp_socket = socket_set.get::<TcpSocket<'_>>(self.socket);
                if tcp_socket.state() == TcpState::Established {
                    self.state = State::Connected;
                    let ep = to_m3_ep(tcp_socket.remote_endpoint());
                    Some(SendNetEvent::Connected(event::ConnectedMessage::new(ep)))
                }
                else if let Some(start) = self.connect_start {
                    if TimeInstant::now() >= start + CONNECT_TIMEOUT {
                        tcp_socket.abort();
                        self._local_port = None;
                        self.state = State::Closed;
                        self.send_queue.clear();
                        Some(SendNetEvent::Closed(event::ClosedMessage::default()))
                    }
                    else {
                        None
                    }
                }
                else {
                    None
                }
            },

            (SocketType::Stream, State::Connected) => {
                let tcp_socket = socket_set.get::<TcpSocket<'_>>(self.socket);
                if !tcp_socket.is_open() {
                    self._local_port = None;
                    self.state = State::Closed;
                    self.send_queue.clear();
                    Some(SendNetEvent::Closed(event::ClosedMessage::default()))
                }
                // remote side has closed the connection?
                else if tcp_socket.state() == TcpState::CloseWait {
                    Some(SendNetEvent::CloseReq(event::CloseReqMessage::default()))
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
        addr: IpAddress,
        port: AnyPort,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Dgram {
            return Err(Error::new(Code::InvArgs));
        }
        if self.state != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        let endpoint = IpEndpoint::new(addr, port.number());
        let mut udp_socket = socket_set.get::<UdpSocket<'_>>(self.socket);
        match udp_socket.bind(endpoint) {
            Ok(_) => {
                if let AnyPort::Ephemeral(e) = port {
                    self._local_port = Some(e);
                }
                self.state = State::Bound;
                Ok(())
            },
            Err(e) => {
                log!(crate::LOG_ERR, "bind failed: {}", e);
                // bind can only fail if the port is zero
                Err(Error::new(Code::InvArgs))
            },
        }
    }

    pub fn listen(
        &mut self,
        socket_set: &mut SocketSet<'static>,
        addr: IpAddress,
        port: Port,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            return Err(Error::new(Code::InvArgs));
        }
        if self.state != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        let endpoint = IpEndpoint::new(addr, port);
        let mut tcp_socket = socket_set.get::<TcpSocket<'_>>(self.socket);
        match tcp_socket.listen(endpoint) {
            Ok(_) => {
                self.connect_start = None;
                self.state = State::Connecting;
                Ok(())
            },
            Err(e) => {
                log!(crate::LOG_ERR, "listen failed: {}", e);
                // listen can only fail if the port is zero
                Err(Error::new(Code::InvArgs))
            },
        }
    }

    pub fn connect(
        &mut self,
        remote_addr: IpAddr,
        remote_port: Port,
        local_port: EphemeralPort,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            return Err(Error::new(Code::InvArgs));
        }
        if self.state != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        let remote_endpoint = IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address::from_bytes(&remote_addr.0.to_be_bytes())),
            remote_port,
        );
        let local_endpoint = IpEndpoint::from(*local_port);

        let mut tcp_socket = socket_set.get::<TcpSocket<'_>>(self.socket);
        match tcp_socket.connect(remote_endpoint, local_endpoint) {
            Ok(_) => {
                self.connect_start = Some(TimeInstant::now());
                self.state = State::Connecting;
                self._local_port = Some(local_port);
                Ok(())
            },
            Err(e) => {
                log!(crate::LOG_ERR, "connect failed: {}", e);
                // connect can only fail if the endpoints are invalid
                Err(Error::new(Code::InvArgs))
            },
        }
    }

    pub fn close(&mut self, socket_set: &mut SocketSet<'static>) -> Result<(), Error> {
        if self.ty != SocketType::Stream {
            return Err(Error::new(Code::InvArgs));
        }

        let mut tcp_socket = socket_set.get::<TcpSocket<'_>>(self.socket);
        tcp_socket.close();
        Ok(())
    }

    pub fn abort(&mut self, socket_set: &mut SocketSet<'static>) {
        if self.ty == SocketType::Stream {
            let mut tcp_socket = socket_set.get::<TcpSocket<'_>>(self.socket);
            tcp_socket.abort();
        }

        self._local_port = None;
        self.state = State::Closed;
    }

    pub fn receive<F>(&mut self, socket_set: &mut SocketSet<'static>, func: F)
    where
        F: FnOnce(&[u8], IpEndpoint) -> usize,
    {
        match self.ty {
            SocketType::Stream => {
                let mut tcp_socket = socket_set.get::<TcpSocket<'_>>(self.socket);
                if self.state == State::Connected {
                    let addr = tcp_socket.remote_endpoint();
                    // don't even log errors here, since they occur often and are uninteresting
                    tcp_socket
                        .recv(|d| {
                            if !d.is_empty() {
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
                let mut udp_socket = socket_set.get::<UdpSocket<'_>>(self.socket);
                if let Ok((data, remote_endpoint)) = udp_socket.recv() {
                    func(data, remote_endpoint);
                }
            },

            SocketType::Raw => {
                let mut raw_socket = socket_set.get::<RawSocket<'_>>(self.socket);
                if let Ok(data) = raw_socket.recv() {
                    func(data, IpEndpoint::UNSPECIFIED);
                }
            },

            SocketType::Undefined => panic!("cannot receive from undefined socket"),
        }
    }

    fn send(
        ty: SocketType,
        socket: SocketHandle,
        data: &[u8],
        dest_addr: IpAddr,
        dest_port: Port,
        socket_set: &mut SocketSet<'static>,
    ) -> usize {
        match ty {
            SocketType::Stream => {
                let mut tcp_socket = socket_set.get::<TcpSocket<'_>>(socket);
                if tcp_socket.can_send() {
                    tcp_socket.send_slice(data).unwrap()
                }
                else {
                    0
                }
            },

            SocketType::Dgram => {
                let mut udp_socket = socket_set.get::<UdpSocket<'_>>(socket);
                if udp_socket.can_send() {
                    let rend = IpEndpoint::new(
                        IpAddress::Ipv4(Ipv4Address::from_bytes(&dest_addr.0.to_be_bytes())),
                        dest_port,
                    );

                    udp_socket.send_slice(data, rend).unwrap();
                    data.len()
                }
                else {
                    0
                }
            },

            SocketType::Raw => {
                let mut raw_socket = socket_set.get::<RawSocket<'_>>(socket);
                if raw_socket.can_send() {
                    raw_socket.send_slice(data).unwrap();
                    data.len()
                }
                else {
                    0
                }
            },

            SocketType::Undefined => panic!("cannot send to undefined socket"),
        }
    }

    pub fn process_queued_events(
        &mut self,
        sess: u64,
        socket_set: &mut SocketSet<'static>,
    ) -> bool {
        let socket = self.socket;
        let ty = self.ty;
        let sd = self.sd;
        #[allow(clippy::blocks_in_if_conditions)]
        while self
            .send_queue
            .next_data(usize::MAX, &mut |data, ep: Endpoint| {
                let amount = Self::send(ty, socket, data, ep.addr, ep.port, socket_set);
                if amount > 0 {
                    log!(
                        crate::LOG_DATA,
                        "[{}] socket {}: sent delayed packet of {}b to {}",
                        sess,
                        sd,
                        amount,
                        ep,
                    );
                }
                (amount, amount)
            })
            .is_some()
        {}
        self.send_queue.has_data()
    }

    pub fn process_event(
        &mut self,
        sess: u64,
        socket_set: &mut SocketSet<'static>,
        event: NetEvent,
    ) -> bool {
        match event.msg_type() {
            NetEventType::DATA => {
                let data = event.msg::<event::DataMessage>();
                let ip = IpAddr(data.addr as u32);
                let port = data.port as Port;

                let res = Self::send(
                    self.ty,
                    self.socket,
                    &data.data[0..data.size as usize],
                    ip,
                    port,
                    socket_set,
                );
                if res > 0 {
                    log!(
                        crate::LOG_DATA,
                        "[{}] socket {}: sent packet of {}b to {}:{}",
                        sess,
                        self.sd,
                        res,
                        ip,
                        port,
                    );
                }

                if res < data.size as usize {
                    // if insufficient buffer space is available, remember the event for later
                    log!(
                        crate::LOG_DATA,
                        "[{}] socket {}: no buffer space, delaying send of {}b to {}:{}",
                        sess,
                        self.sd,
                        data.size as usize - res,
                        ip,
                        port,
                    );
                    self.send_queue.append(event, res);
                    return true;
                }
            },

            NetEventType::CLOSE_REQ => {
                log!(crate::LOG_SESS, "[{}] net::close_req(sd={})", sess, self.sd);

                // ignore error
                self.close(socket_set).ok();
            },

            m => log!(crate::LOG_ERR, "Unexpected message from client: {}", m),
        }
        false
    }
}
