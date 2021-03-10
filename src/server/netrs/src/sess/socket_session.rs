/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

use m3::{cap::Selector, net::NetData};
use m3::cell::RefCell;
use m3::col::Vec;
use m3::com::{GateIStream, MGateArgs, MemGate, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::net::{
    net_channel::NetChannel, SocketType, MAX_NETDATA_SIZE, MSG_BUF_ORDER, MSG_BUF_SIZE, MSG_ORDER,
};
use m3::rc::Rc;
use m3::server::CapExchange;
use m3::session::ServerSession;
use m3::tcu;
use m3::vfs::OpenFlags;

use crate::sess::file_session::FileSession;
use crate::sess::sockets::Socket;

use smoltcp;
use smoltcp::socket::SocketSet;
use smoltcp::socket::{
    RawSocket, RawSocketBuffer, TcpSocket, TcpSocketBuffer, UdpSocket, UdpSocketBuffer,
};
use smoltcp::storage::PacketMetadata;
use smoltcp::wire::{IpAddress, IpEndpoint, IpVersion, Ipv4Address};

use super::sockets::{TCP_HEADER_SIZE, UDP_HEADER_SIZE};

pub const MAX_SEND_RECEIVE_BATCH_SIZE: usize = 5;
pub const MAX_SOCKETS: usize = 16;
///Defines how big the socket buffers must be, currently this is the max NetDataSize multiplied by the
/// Maximum in flight packages
pub const TCP_BUFFER_SIZE: usize =
    (MAX_NETDATA_SIZE + TCP_HEADER_SIZE) * MAX_SEND_RECEIVE_BATCH_SIZE;
pub const UDP_BUFFER_SIZE: usize =
    (MAX_NETDATA_SIZE + UDP_HEADER_SIZE) * MAX_SEND_RECEIVE_BATCH_SIZE;
pub const RAW_BUFFER_SIZE: usize = MAX_NETDATA_SIZE * MAX_SEND_RECEIVE_BATCH_SIZE;

pub struct SocketSession {
    sgate: Option<SendGate>,
    rgate: Rc<RefCell<RecvGate>>,
    server_session: ServerSession,
    sockets: Vec<Option<Rc<RefCell<Socket>>>>, //All sockets registered to this socket session.
    ///Size of the memory gate that can be used to transfer buffers
    size: usize,
    ///Used to communicate with the client
    channel: Option<NetChannel>,
    ///Capabilities start, saved for revoking purpose
    channel_caps: Selector,
    ///Only used to keep the gates used by the client alive
    client_gates: Option<(SendGate, RecvGate, MemGate)>,
}

impl Drop for SocketSession {
    fn drop(&mut self) {
        for i in 0..MAX_SOCKETS {
            self.release_sd(i as i32)
        }
        if self.channel_caps != m3::kif::INVALID_SEL {
            m3::pes::VPE::cur()
                .revoke(
                    m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, self.channel_caps, 6),
                    false,
                )
                .expect("Failed to revoke caps of socket session");
        }
    }
}

impl SocketSession {
    pub fn new(_crt: usize, server_session: ServerSession, rgate: Rc<RefCell<RecvGate>>) -> Self {
        SocketSession {
            sgate: None,
            rgate,
            server_session,
            sockets: vec![None; MAX_SOCKETS], //TODO allocate correct amount up front?
            size: TCP_BUFFER_SIZE,            //currently going with the max number
            channel: None,
            channel_caps: m3::kif::INVALID_SEL,
            client_gates: None,
        }
    }

    pub fn obtain(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        xchg: &mut CapExchange,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "SocketSession::obtain with {} in caps",
            xchg.in_caps()
        );

        if xchg.in_caps() == 1 {
            self.get_sgate(xchg)
        }
        else if xchg.in_caps() == 3 {
            //establish a connection with the network manager in that session
            self.connect_nm(xchg)
        }
        else if xchg.in_caps() == 2 {
            self.open_file(crt, srv_sel, xchg)
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    pub fn rgate(&self) -> Rc<RefCell<RecvGate>> {
        self.rgate.clone()
    }

    ///Creates a SendGate that is used to send data to this socket session.
    ///keeps the Sendgate alive and sends back the selector that needs to be binded to.
    fn get_sgate(&mut self, xchg: &mut CapExchange) -> Result<(), Error> {
        if self.sgate.is_some() {
            return Err(Error::new(Code::InvArgs));
        }

        let label = self.server_session.ident() as tcu::Label;

        log!(
            crate::LOG_DEF,
            "SocketSession::get_sgate with label={}",
            label
        );

        self.sgate = Some(SendGate::new_with(
            m3::com::SGateArgs::new(&self.rgate.borrow())
                .label(label)
                .credits(1),
        )?);

        xchg.out_caps(m3::kif::CapRngDesc::new(
            m3::kif::CapType::OBJECT,
            self.sgate.as_ref().unwrap().sel(),
            1,
        ));
        Ok(())
    }

    fn connect_nm(&mut self, xchg: &mut CapExchange) -> Result<(), Error> {
        log!(crate::LOG_DEF, "Establishing channel for socket session");

        //establishes the channel by creating the send/recv gates
        // src->client
        // client->server

        //0: rgate_srv,
        //1: sgate_srv,
        //2: mem_srv

        //3: rgate_cli,
        //4: sgate_cli
        //5: mem_cli
        let caps = m3::pes::VPE::cur().alloc_sels(6);

        self.channel_caps = caps;

        //Create the local channel, but also keep the client data alive so the client can bind to them.
        let rgate_srv = RecvGate::new_with(
            RGateArgs::default()
                .order(MSG_BUF_ORDER)
                .msg_order(MSG_ORDER)
                .sel(caps + 0),
        )?;
        let rgate_cli = RecvGate::new_with(
            RGateArgs::default()
                .order(MSG_BUF_ORDER)
                .msg_order(MSG_ORDER)
                .sel(caps + 3),
        )?;
        let sgate_srv = SendGate::new_with(SGateArgs::new(&rgate_cli).sel(caps + 1))?; //reply gate, flags and credits?
        let sgate_cli = SendGate::new_with(SGateArgs::new(&rgate_srv).sel(caps + 4))?;

        let mem_srv =
            MemGate::new_with(MGateArgs::new(2 * self.size, m3::kif::Perm::RW).sel(caps + 2))?;
        let mem_cli = mem_srv.derive_for(
            m3::pes::VPE::cur().sel(),
            caps + 5,
            0,
            2 * self.size,
            m3::kif::Perm::RW,
        )?;

        //Create local channel end and store the rest
        self.channel = Some(NetChannel::new_with_gates(sgate_srv, rgate_srv, mem_srv));
        self.client_gates = Some((sgate_cli, rgate_cli, mem_cli));

        //Send capabilities back to caller so it can connect to the created gates
        xchg.out_caps(m3::kif::CapRngDesc::new(
            m3::kif::CapType::OBJECT,
            caps + 3,
            3,
        ));

        Ok(())
    }

    fn open_file(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        xchg: &mut CapExchange,
    ) -> Result<(), Error> {
        let sd = xchg.in_args().pop::<i32>().expect("Failed to get sd");
        let mode = xchg.in_args().pop::<u32>().expect("Failed to get mode");
        let rmemsize = xchg
            .in_args()
            .pop::<usize>()
            .expect("Failed to get rmemsize");
        let smemsize = xchg
            .in_args()
            .pop::<usize>()
            .expect("Failed to get smemsize");

        log!(
            crate::LOG_DEF,
            "socket_session::open_file(sd={}, mode={}, rmemsize={}, smemsize={})",
            sd,
            mode,
            rmemsize,
            smemsize
        );
        //Create socket for file
        if let Some(socket) = self.get_socket(sd) {
            if (mode & OpenFlags::RW.bits()) == 0 {
                log!(crate::LOG_DEF, "open_file failed: invalid mode");
                return Err(Error::new(Code::InvArgs));
            }

            if (socket.borrow().rfile.is_some() && ((mode & OpenFlags::R.bits()) > 0))
                || (socket.borrow().sfile.is_some() && ((mode & OpenFlags::W.bits()) > 0))
            {
                log!(
                    crate::LOG_DEF,
                    "open_file failed: socket already has a file session attached"
                );
                return Err(Error::new(Code::InvArgs));
            }
            let file = FileSession::new(crt, srv_sel, socket.clone(), mode, rmemsize, smemsize)?;
            if file.borrow().is_recv() {
                socket.borrow_mut().rfile = Some(file.clone());
            }
            if file.borrow().is_send() {
                socket.borrow_mut().sfile = Some(file.clone());
            }

            socket.borrow_mut().rgate = Some(self.rgate.clone());
            xchg.out_caps(file.borrow().caps());

            log!(
                crate::LOG_DEF,
                "open_file: {}@{}{}",
                sd,
                if file.borrow().is_recv() { "r" } else { "" },
                if file.borrow().is_send() { "s" } else { "" }
            );
            Ok(())
        }
        else {
            log!(
                crate::LOG_DEF,
                "open_file failed: invalud socket descriptor"
            );
            Err(Error::new(Code::InvArgs))
        }
    }

    fn get_socket(&self, sd: i32) -> Option<Rc<RefCell<Socket>>> {
        if let Some(s) = self.sockets.get(sd as usize) {
            s.clone()
        }
        else {
            None
        }
    }

    fn remove_socket(&mut self, sd: i32) {
        //if there is a socket, delete it.
        if self.sockets.get(sd as usize).is_some() {
            self.sockets[sd as usize] = None;
        }
    }

    fn request_sd(&mut self, mut socket: Socket) -> Result<i32, Error> {
        for (i, s) in self.sockets.iter_mut().enumerate() {
            if s.is_none() {
                socket.sd = i as i32;
                *s = Some(Rc::new(RefCell::new(socket)));
                return Ok(i as i32);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    fn release_sd(&mut self, sd: i32) {
        debug_assert!(sd >= 0, "sd should be bigger then 0!");
        self.sockets[sd as usize] = None;
    }

    pub fn create(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let ty_id: usize = is.pop()?;
        let ty = SocketType::from_usize(ty_id);
        let protocol: u8 = is.pop()?;

        log!(
            crate::LOG_DEF,
            "net::create(type={:?}, protocol={})",
            ty,
            protocol
        );

        let socket_handle = match ty {
            SocketType::Stream => {
                self.size = TCP_BUFFER_SIZE;
                socket_set.add(TcpSocket::new(
                    TcpSocketBuffer::new(vec![0 as u8; TCP_BUFFER_SIZE]),
                    TcpSocketBuffer::new(vec![0 as u8; TCP_BUFFER_SIZE]),
                ))
            },
            SocketType::Dgram => {
                self.size = UDP_BUFFER_SIZE;
                socket_set.add(UdpSocket::new(
                    UdpSocketBuffer::new(
                        vec![PacketMetadata::EMPTY; MAX_SEND_RECEIVE_BATCH_SIZE],
                        vec![0 as u8; UDP_BUFFER_SIZE],
                    ),
                    UdpSocketBuffer::new(
                        vec![PacketMetadata::EMPTY; MAX_SEND_RECEIVE_BATCH_SIZE],
                        vec![0 as u8; UDP_BUFFER_SIZE],
                    ),
                ))
            },
            SocketType::Raw => {
                self.size = RAW_BUFFER_SIZE;
                socket_set.add(RawSocket::new(
                    IpVersion::Ipv4,
                    protocol.into(),
                    RawSocketBuffer::new(vec![PacketMetadata::EMPTY; MSG_BUF_SIZE], vec![
			0 as u8;
			RAW_BUFFER_SIZE
                    ]),
                    RawSocketBuffer::new(vec![PacketMetadata::EMPTY; MSG_BUF_SIZE], vec![
			0 as u8;
			RAW_BUFFER_SIZE
                    ]),
                ))
            },
            _ => {
                log!(crate::LOG_DEF, "create failed: invalid socket type");
                return Err(Error::new(Code::InvArgs));
            },
        };

        //Create the abstract socket from some created socket instance
        let socket = Socket::from_smol_socket(socket_handle, ty, self.rgate.clone());
        let sd = match self.request_sd(socket) {
            Ok(sd) => sd,
            Err(_e) => {
                //TODO release socket
                log!(
                    crate::LOG_DEF,
                    "create failed: maximum number of sockets reached"
                );
                return Err(Error::new(Code::NoSpace));
            },
        };

        log!(crate::LOG_DEF, "-> sd={}", sd);
        reply_vmsg!(is, 0 as u32, sd)
    }

    pub fn bind(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: i32 = is.pop()?;
        let addr: u32 = is.pop()?;
        let port: u16 = is.pop()?;

        let endpoint = IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.to_be_bytes())),
            port,
        );

        log!(
            crate::LOG_DEF,
            "net::bind(sd={}, addr={}, port={})",
            sd,
            endpoint.addr,
            endpoint.port
        );

        if let Some(sock) = self.get_socket(sd) {
            //TODO verify that the bigEndian is indeed the correct byte order
            sock.borrow_mut().bind(endpoint, socket_set)?;
            reply_vmsg!(is, Code::None as i32)
        }
        else {
            log!(crate::LOG_DEF, "bind failed, invalid socket descriptor");
            Err(Error::new(Code::NoSpace))
        }
    }

    pub fn listen(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: i32 = is.pop()?;
        let addr: u32 = is.pop()?;
        let port: u16 = is.pop()?;
        let endpoint = IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.to_be_bytes())),
            port,
        );

        log!(
            crate::LOG_DEF,
            "net::listen(sd={}, local_addr={}, local_port={})",
            sd,
            endpoint.addr,
            endpoint.port
        );

        if let Some(socket) = self.get_socket(sd) {
            socket.borrow_mut().listen(socket_set, endpoint)?;
            reply_vmsg!(is, Code::None as i32)
        }
        else {
            log!(crate::LOG_DEF, "listen failed: invalud socket descriptor");
            Err(Error::new(Code::NoSpace))
        }
    }

    pub fn connect(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: i32 = is.pop()?;
        let remote_addr: u32 = is.pop()?;
        let remote_port: u16 = is.pop()?;
        let local_addr: u32 = is.pop()?;
        let local_port: u16 = is.pop()?;

        let remote_endpoint = IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address::from_bytes(&remote_addr.to_be_bytes())),
            remote_port,
        );
        let local_endpoint = IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address::from_bytes(&local_addr.to_be_bytes())),
            local_port,
        );
        log!(
            crate::LOG_DEF,
            "net::connect(sd={}, remote={}, local={})",
            sd,
            remote_endpoint,
            local_endpoint
        );

        if let Some(sock) = self.get_socket(sd) {
            //TODO verify that the bigEndian is indeed the correct byte order
            sock.borrow_mut()
                .connect(remote_endpoint, local_endpoint, socket_set)?;
            reply_vmsg!(is, Code::None as i32) //all went good
        }
        else {
            log!(crate::LOG_DEF, "connect failed: invalid socket descriptor");
            Err(Error::new(Code::NoSpace))
        }
    }

    pub fn close(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: i32 = is.pop()?;
        log!(crate::LOG_DEF, "net::close(sd={})", sd);

        if let Some(socket) = self.get_socket(sd) {
            socket.borrow_mut().close(socket_set)?;
            self.remove_socket(sd);
            reply_vmsg!(is, Code::None as i32)
        }
        else {
            log!(crate::LOG_DEF, "close failed: invalid socket descriptor");
            Err(Error::new(Code::NoSpace))
        }
    }

    pub fn send(&mut self, socket_set: &mut SocketSet<'static>) {
        if self.channel.is_none() {
            //Cannot send yet since the channel is not active.
            return;
        }

        let mut num_received = 0;

        //receive everything in the channel
        while let Ok(data) = self.channel.as_ref().unwrap().receive() {
            num_received += 1;

            if let Some(socket) = self.get_socket(data.sd) {
                log!(
                    crate::LOG_DEF,
                    "DataAsString={}",
                    core::str::from_utf8(data.raw_data()).unwrap_or("Could not parse")
                );
                let _send_data_size = match socket.borrow_mut().send_data_slice(data, socket_set) {
                    Ok(send_size) => send_size,
                    Err(e) => {
                        log!(
                            crate::LOG_DEF,
                            "Failed to send data over smoltcp_socket: {}",
                            e
                        );
                        0
                    },
                };
            //TODO return send size if it is blocking?
            }
            else {
                log!(
                    crate::LOG_DEF,
                    "send failed: invalid socket descriptor [{}]",
                    data.sd
                );
            }

            if num_received > MAX_SEND_RECEIVE_BATCH_SIZE {
                break;
            }
        }
    }

    ///Ticks this socket. If there was a package to receive, puts it onto the netChannel to be consumed by some client.
    pub fn receive(&mut self, socket_set: &mut SocketSet<'static>) {
        if self.channel.is_none() {
            //Cannot receive yet since the channel is not active.
            return;
        }
        //iterate over all sockets and try to receive
        for socket in self.sockets.iter() {
            if let Some(socket) = socket {
                let socket_sd = socket.borrow().sd;
                if let Ok(mut net_data) = socket.borrow_mut().receive(socket_set) {
                    //Drop if no data was send. In that case this was some communication
                    //package between sockets
                    if net_data.size == 0 {
                        continue;
                    }

                    log!(
                        crate::LOG_DEF,
                        "Received package with size={} from {}:{}",
                        net_data.size,
                        net_data.source_addr,
                        net_data.source_port
                    );

                    //set the packages socket descriptor
                    net_data.sd = socket_sd;

                    if let Err(e) = self.channel.as_ref().unwrap().send(net_data) {
                        log!(
                            crate::LOG_DEF,
                            "Failed to send received package over channel to client: {}",
                            e
                        );
                    }
                }
            }
        }
    }

    pub fn query_state(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: i32 = is.pop()?;
        if let Some(socket) = self.get_socket(sd) {
            let state = socket.borrow().get_state(socket_set)?;
            log!(crate::LOG_DEF, "net::state: State is: {:?}", state);
            //send state back
            reply_vmsg!(is, Code::None as i32, state)
        }
        else {
            Err(Error::new(Code::NotSup)) //TODO change back into "NoSuchSocket"
        }
    }
}
