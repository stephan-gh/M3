/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

mod settings;

use core::cmp;

use base::io::LogFlags;

use m3::cap::{SelSpace, Selector};
use m3::cell::RefCell;
use m3::col::Vec;
use m3::com::GateIStream;
use m3::errors::{Code, Error};
use m3::kif::{CapRngDesc, CapType};
use m3::net::{log_net, IpAddr, NetLogEvent, Port, Sd, SocketArgs, SocketType, MTU};
use m3::rc::Rc;
use m3::server::{CapExchange, RequestSession, ServerSession};
use m3::{log, reply_vmsg, vec};

use crate::driver::DriverInterface;
use crate::ports::{self, AnyPort};
use crate::smoltcpif::socket::{to_m3_addr, to_m3_ep, SendNetEvent, Socket};

pub struct SocketSession {
    // our session cap
    serv: ServerSession,
    // the settings for this session
    settings: settings::Settings,
    // sockets the client has open
    sockets: Vec<Option<Rc<RefCell<Socket>>>>,
}

impl RequestSession for SocketSession {
    fn new(serv: ServerSession, arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        log!(LogFlags::NetSess, "[{}] net::open(arg={})", serv.id(), arg,);

        let settings = settings::parse_arguments(arg)?;
        Ok(SocketSession {
            serv,
            sockets: vec![None; settings.socks],
            settings,
        })
    }

    fn close(
        &mut self,
        _cli: &mut m3::server::ClientManager<Self>,
        sid: m3::server::SessId,
        _sub_ids: &mut Vec<m3::server::SessId>,
    ) where
        Self: Sized,
    {
        log!(LogFlags::NetSess, "[{}] net::close()", sid);
    }
}

impl SocketSession {
    pub fn create_socket(
        &mut self,
        xchg: &mut CapExchange<'_>,
        iface: &mut DriverInterface<'_>,
    ) -> Result<(), Error> {
        let is = xchg.in_args();

        let ty = SocketType::from_usize(is.pop::<usize>()?);
        let protocol: u8 = is.pop()?;
        let rbuf_size: usize = is.pop()?;
        let rbuf_slots: usize = is.pop()?;
        let sbuf_size: usize = is.pop()?;
        let sbuf_slots: usize = is.pop()?;

        // 2 caps for us, 2 for the client
        let caps = SelSpace::get().alloc_sels(4);

        let res = self.add_socket(
            ty,
            protocol,
            &SocketArgs {
                rbuf_slots,
                rbuf_size,
                sbuf_slots,
                sbuf_size,
            },
            caps,
            iface,
        );

        log!(
            LogFlags::NetSess,
            "net::create(type={:?}, protocol={}, rbuf=[{}b,{}], sbuf=[{}b,{}]) -> {:?}",
            ty,
            protocol,
            rbuf_size,
            rbuf_slots,
            sbuf_size,
            sbuf_slots,
            res
        );

        match res {
            Ok(sd) => {
                // Send capabilities back to caller so it can connect to the created gates
                xchg.out_caps(CapRngDesc::new(CapType::Object, caps + 2, 2));
                xchg.out_args().push(sd);
                Ok(())
            },

            Err(e) => Err(e),
        }
    }

    fn get_socket(&self, sd: Sd) -> Result<Rc<RefCell<Socket>>, Error> {
        match self.sockets.get(sd) {
            Some(Some(s)) => Ok(s.clone()),
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    fn add_socket(
        &mut self,
        ty: SocketType,
        protocol: u8,
        args: &SocketArgs,
        caps: Selector,
        iface: &mut DriverInterface<'_>,
    ) -> Result<Sd, Error> {
        if ty == SocketType::Raw && !self.settings.raw {
            return Err(Error::new(Code::NoPerm));
        }

        let total_space = Socket::required_space(ty, args);
        if self.settings.bufs < total_space {
            return Err(Error::new(Code::NoSpace));
        }

        for (i, s) in self.sockets.iter_mut().enumerate() {
            if s.is_none() {
                *s = Some(Rc::new(RefCell::new(Socket::new(
                    i, ty, protocol, args, caps, iface,
                )?)));
                self.settings.bufs -= total_space;
                return Ok(i);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    fn remove_socket(&mut self, sd: Sd) {
        if let Some(s) = self.sockets[sd].take() {
            self.settings.bufs += s.borrow().buffer_space();
        }
    }

    fn can_use_port(&self, ty: SocketType, port: Port) -> bool {
        let ports = match ty {
            SocketType::Stream => &self.settings.tcp_ports,
            SocketType::Dgram => &self.settings.udp_ports,
            _ => return true,
        };
        for range in ports {
            if port >= range.0 && port <= range.1 {
                return true;
            }
        }
        false
    }

    pub fn bind(
        &mut self,
        is: &mut GateIStream<'_>,
        iface: &mut DriverInterface<'_>,
    ) -> Result<(), Error> {
        let sd: Sd = is.pop()?;
        let port: Port = is.pop()?;

        log!(
            LogFlags::NetSess,
            "[{}] net::bind(sd={}, port={})",
            self.serv.id(),
            sd,
            port
        );

        let sock = self.get_socket(sd)?;
        let port = if port == 0 {
            AnyPort::Ephemeral(ports::alloc())
        }
        else {
            if !self.can_use_port(sock.borrow().socket_type(), port) {
                return Err(Error::new(Code::NoPerm));
            }

            AnyPort::Manual(port)
        };

        let port_no = port.number();
        sock.borrow_mut().bind(crate::own_ip(), port, iface)?;

        let addr = to_m3_addr(crate::own_ip());
        reply_vmsg!(is, Code::Success, addr.0, port_no)
    }

    pub fn listen(
        &mut self,
        is: &mut GateIStream<'_>,
        iface: &mut DriverInterface<'_>,
    ) -> Result<(), Error> {
        let sd: Sd = is.pop()?;
        let port: Port = is.pop()?;

        log!(
            LogFlags::NetSess,
            "[{}] net::listen(sd={}, port={})",
            self.serv.id(),
            sd,
            port
        );

        let sock = self.get_socket(sd)?;
        if !self.can_use_port(sock.borrow().socket_type(), port) {
            return Err(Error::new(Code::NoPerm));
        }

        sock.borrow_mut().listen(iface, crate::own_ip(), port)?;

        let addr = to_m3_addr(crate::own_ip());
        reply_vmsg!(is, Code::Success, addr.0)
    }

    pub fn connect(
        &mut self,
        is: &mut GateIStream<'_>,
        iface: &mut DriverInterface<'_>,
    ) -> Result<(), Error> {
        let sd: Sd = is.pop()?;
        let remote_addr = IpAddr(is.pop::<u32>()?);
        let remote_port: Port = is.pop()?;

        let local_port = ports::alloc();
        log!(
            LogFlags::NetSess,
            "[{}] net::connect(sd={}, remote={}:{}, local={})",
            self.serv.id(),
            sd,
            remote_addr,
            remote_port,
            local_port
        );

        let sock = self.get_socket(sd)?;
        let port_no = *local_port;
        sock.borrow_mut()
            .connect(remote_addr, remote_port, local_port, iface)?;

        let addr = to_m3_addr(crate::own_ip());
        reply_vmsg!(is, Code::Success, addr.0, port_no)
    }

    pub fn abort(
        &mut self,
        is: &mut GateIStream<'_>,
        iface: &mut DriverInterface<'_>,
    ) -> Result<(), Error> {
        let sd: Sd = is.pop()?;
        let remove: bool = is.pop()?;

        self.do_abort(sd, remove, iface)?;
        is.reply_error(Code::Success)
    }

    pub fn abort_all(&mut self, iface: &mut DriverInterface<'_>) -> Result<(), Error> {
        for sd in 0..self.sockets.len() {
            self.do_abort(sd, true, iface).ok();
        }
        Ok(())
    }

    fn do_abort(
        &mut self,
        sd: Sd,
        remove: bool,
        iface: &mut DriverInterface<'_>,
    ) -> Result<(), Error> {
        log!(
            LogFlags::NetSess,
            "[{}] net::abort(sd={}, remove={})",
            self.serv.id(),
            sd,
            remove
        );

        let socket = self.get_socket(sd)?;
        socket.borrow_mut().abort(iface);
        if remove {
            self.remove_socket(sd);
        }
        Ok(())
    }

    pub fn process_incoming(&mut self, iface: &mut DriverInterface<'_>) -> bool {
        let sess = self.serv.id();
        let mut needs_recheck = false;

        // iterate over all sockets and check for events
        'outer_loop: for idx in 0..self.sockets.len() {
            if let Some(socket) = self.sockets.get(idx).unwrap() {
                let mut sock = socket.borrow_mut();
                let chan = sock.channel().clone();

                chan.fetch_replies();

                if sock.process_queued_events(sess, iface) {
                    needs_recheck = true;
                    continue 'outer_loop;
                }

                // receive everything in the channel
                while let Some(event) = chan.fetch_event() {
                    if sock.process_event(sess, iface, event) {
                        needs_recheck = true;
                        continue 'outer_loop;
                    }
                }
            }
        }

        needs_recheck
    }

    pub fn process_outgoing(&mut self, iface: &mut DriverInterface<'_>) -> bool {
        let mut needs_recheck = false;
        // iterate over all sockets and try to receive
        for socket in self.sockets.iter().flatten() {
            let socket_sd = socket.borrow().sd();
            let chan = socket.borrow().channel().clone();

            loop {
                chan.fetch_replies();

                // if we don't have credits anymore to send events, stop here. we'll get a reply
                // to one of our earlier events and get credits back with this, so that we'll wake
                // up from a potential sleep and call receive again.
                if !chan.can_send().unwrap() {
                    needs_recheck = true;
                    break;
                }

                if let Some(event) = socket.borrow_mut().fetch_event(iface) {
                    log!(
                        LogFlags::NetData,
                        "[{}] socket {}: received event {:?}",
                        socket_sd,
                        self.serv.id(),
                        event,
                    );

                    // the match is needed, because we don't want to send the enum, but the
                    // contained event struct
                    match event {
                        SendNetEvent::Connected(e) => {
                            log_net(
                                NetLogEvent::RecvConnected,
                                socket_sd,
                                e.remote_port as usize,
                            );
                            chan.send_event(e).unwrap()
                        },
                        SendNetEvent::Closed(e) => {
                            log_net(NetLogEvent::RecvClosed, socket_sd, 0);
                            chan.send_event(e).unwrap()
                        },
                        SendNetEvent::CloseReq(e) => {
                            log_net(NetLogEvent::RecvRemoteClosed, socket_sd, 0);
                            chan.send_event(e).unwrap()
                        },
                    }
                }

                if !chan.can_send().unwrap() {
                    needs_recheck = true;
                    break;
                }

                let mut received = false;
                socket.borrow_mut().receive(iface, |data, addr| {
                    let ep = to_m3_ep(addr);
                    let amount = cmp::min(MTU, data.len());

                    log_net(NetLogEvent::FetchData, socket_sd, amount);
                    log!(
                        LogFlags::NetData,
                        "[{}] socket {}: received packet with {}b from {}",
                        socket_sd,
                        self.serv.id(),
                        amount,
                        ep
                    );

                    let msg = chan.build_data_message(ep, amount, |buf| {
                        buf[0..amount].copy_from_slice(&data[0..amount]);
                    });

                    if let Err(e) = chan.send_data(&msg) {
                        log!(
                            LogFlags::Error,
                            "[{}] socket {}: sending received packet with {}b failed: {}",
                            socket_sd,
                            self.serv.id(),
                            amount,
                            e
                        );
                    }
                    received = true;
                    amount
                });

                if !received {
                    break;
                }
            }
        }
        needs_recheck
    }
}
