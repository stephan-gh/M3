/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

use core::cmp;

use m3::cap::Selector;
use m3::cell::RefCell;
use m3::col::Vec;
use m3::com::{GateIStream, RecvGate, SendGate};
use m3::errors::{Code, Error};
use m3::kif::{CapRngDesc, CapType};
use m3::net::{event, IpAddr, Port, Sd, SocketArgs, SocketType};
use m3::parse;
use m3::rc::Rc;
use m3::serialize::Source;
use m3::server::CapExchange;
use m3::session::{NetworkOp, ServerSession};
use m3::tcu;
use m3::vfs::OpenFlags;
use m3::{log, reply_vmsg, vec};

use smoltcp::socket::SocketSet;

use crate::ports;
use crate::sess::file::FileSession;
use crate::smoltcpif::socket::{to_m3_addr, to_m3_ep, SendNetEvent, Socket};

struct Args {
    bufs: usize,
    socks: usize,
    ports: Vec<(Port, Port)>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            bufs: 64 * 1024,
            socks: 4,
            ports: Vec::new(),
        }
    }
}

fn parse_arguments(args_str: &str) -> Result<Args, Error> {
    let mut args = Args::default();
    for arg in args_str.split_whitespace() {
        if let Some(bufs) = arg.strip_prefix("bufs=") {
            args.bufs = parse::size(bufs)?;
        }
        else if let Some(socks) = arg.strip_prefix("socks=") {
            args.socks = parse::int(socks)? as usize;
        }
        else if let Some(portdesc) = arg.strip_prefix("ports=") {
            // comma separated list of "x-y" or "x"
            for ports in portdesc.split(',') {
                if let Some(pos) = ports.find('-') {
                    let from = parse::int(&ports[0..pos])? as Port;
                    let to = parse::int(&ports[(pos + 1)..])? as Port;
                    args.ports.push((from, to));
                }
                else {
                    let port = parse::int(&ports)? as Port;
                    args.ports.push((port, port));
                }
            }
        }
        else {
            return Err(Error::new(Code::InvArgs));
        }
    }
    Ok(args)
}

pub struct SocketSession {
    // client send gate to send us requests
    sgate: Option<SendGate>,
    // our receive gate (shared among all sessions)
    rgate: Rc<RecvGate>,
    // the ports usable for bind and listen
    ports: Vec<(Port, Port)>,
    // the remaining buffer space available to this session
    buf_quota: usize,
    // our session cap
    server_session: ServerSession,
    // sockets the client has open
    sockets: Vec<Option<Rc<RefCell<Socket>>>>,
}

impl SocketSession {
    pub fn new(
        _crt: usize,
        args_str: &str,
        server_session: ServerSession,
        rgate: Rc<RecvGate>,
    ) -> Result<Self, Error> {
        let args = parse_arguments(args_str).map_err(|e| {
            log!(
                crate::LOG_ERR,
                "Unable to parse session arguments: '{}'",
                args_str
            );
            e
        })?;

        for range in &args.ports {
            if ports::is_ephemeral(range.0) || ports::is_ephemeral(range.1) {
                log!(crate::LOG_ERR, "Cannot bind/listen on ephemeral ports");
                return Err(Error::new(Code::InvArgs));
            }
        }

        Ok(SocketSession {
            sgate: None,
            rgate,
            ports: args.ports,
            buf_quota: args.bufs,
            server_session,
            sockets: vec![None; args.socks],
        })
    }

    pub fn obtain(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        xchg: &mut CapExchange,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let is = xchg.in_args();
        let op = is.pop::<NetworkOp>()?;

        match op {
            NetworkOp::GET_SGATE => {
                let caps = self.get_sgate()?;
                xchg.out_caps(caps);
                Ok(())
            },
            NetworkOp::CREATE => {
                let (caps, sd) = self.create_socket(is, socket_set)?;
                xchg.out_caps(caps);
                xchg.out_args().push_word(sd as u64);
                Ok(())
            },
            NetworkOp::OPEN_FILE => {
                let caps = self.open_file(crt, srv_sel, is)?;
                xchg.out_caps(caps);
                Ok(())
            },
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    fn get_sgate(&mut self) -> Result<CapRngDesc, Error> {
        if self.sgate.is_some() {
            return Err(Error::new(Code::InvArgs));
        }

        let label = self.server_session.ident() as tcu::Label;
        self.sgate = Some(SendGate::new_with(
            m3::com::SGateArgs::new(&self.rgate).label(label).credits(1),
        )?);

        Ok(CapRngDesc::new(
            CapType::OBJECT,
            self.sgate.as_ref().unwrap().sel(),
            1,
        ))
    }

    fn open_file(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        is: &mut Source,
    ) -> Result<CapRngDesc, Error> {
        let sd = is.pop::<Sd>().expect("Failed to get sd");
        let mode = is.pop::<u32>().expect("Failed to get mode");
        let rmemsize = is.pop::<usize>().expect("Failed to get rmemsize");
        let smemsize = is.pop::<usize>().expect("Failed to get smemsize");

        log!(
            crate::LOG_SESS,
            "socket_session::open_file(sd={}, mode={}, rmemsize={}, smemsize={})",
            sd,
            mode,
            rmemsize,
            smemsize
        );
        // Create socket for file
        let socket = self.get_socket(sd)?;
        if (mode & OpenFlags::RW.bits()) == 0 {
            log!(crate::LOG_SESS, "open_file failed: invalid mode");
            return Err(Error::new(Code::InvArgs));
        }

        if (socket.borrow().recv_file().is_some() && ((mode & OpenFlags::R.bits()) > 0))
            || (socket.borrow().send_file().is_some() && ((mode & OpenFlags::W.bits()) > 0))
        {
            log!(
                crate::LOG_SESS,
                "open_file failed: socket already has a file session attached"
            );
            return Err(Error::new(Code::InvArgs));
        }
        let file = FileSession::new(
            crt,
            srv_sel,
            socket.clone(),
            &self.rgate,
            mode,
            rmemsize,
            smemsize,
        )?;
        if file.borrow().is_recv() {
            socket.borrow_mut().set_recv_file(Some(file.clone()));
        }
        if file.borrow().is_send() {
            socket.borrow_mut().set_send_file(Some(file.clone()));
        }

        log!(
            crate::LOG_SESS,
            "open_file: {}@{}{}",
            sd,
            if file.borrow().is_recv() { "r" } else { "" },
            if file.borrow().is_send() { "s" } else { "" }
        );

        let caps = file.borrow().caps();
        Ok(caps)
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
        socket_set: &mut SocketSet<'static>,
    ) -> Result<Sd, Error> {
        let total_space = Socket::required_space(ty, args);
        if self.buf_quota < total_space {
            return Err(Error::new(Code::NoSpace));
        }

        for (i, s) in self.sockets.iter_mut().enumerate() {
            if s.is_none() {
                *s = Some(Rc::new(RefCell::new(Socket::new(
                    i, ty, protocol, args, caps, socket_set,
                )?)));
                self.buf_quota -= total_space;
                return Ok(i);
            }
        }
        Err(Error::new(Code::NoSpace))
    }

    fn remove_socket(&mut self, sd: Sd) {
        if let Some(s) = self.sockets[sd].take() {
            self.buf_quota += s.borrow().buffer_space();
        }
    }

    fn create_socket(
        &mut self,
        is: &mut Source,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(CapRngDesc, Sd), Error> {
        let ty = SocketType::from_usize(is.pop::<usize>()?);
        let protocol: u8 = is.pop()?;
        let rbuf_size: usize = is.pop()?;
        let rbuf_slots: usize = is.pop()?;
        let sbuf_size: usize = is.pop()?;
        let sbuf_slots: usize = is.pop()?;

        // 2 caps for us, 2 for the client
        let caps = m3::pes::VPE::cur().alloc_sels(4);

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
            socket_set,
        );

        log!(
            crate::LOG_SESS,
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
                let caps = CapRngDesc::new(CapType::OBJECT, caps + 2, 2);
                Ok((caps, sd))
            },

            Err(e) => Err(e),
        }
    }

    fn can_use_port(&self, port: Port) -> bool {
        for range in &self.ports {
            if port >= range.0 && port <= range.1 {
                return true;
            }
        }
        false
    }

    pub fn bind(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: Sd = is.pop()?;
        let port: Port = is.pop()?;

        log!(
            crate::LOG_SESS,
            "[{}] net::bind(sd={}, port={})",
            self.server_session.ident(),
            sd,
            port
        );

        if !self.can_use_port(port) {
            return Err(Error::new(Code::NoPerm));
        }

        let sock = self.get_socket(sd)?;
        sock.borrow_mut()
            .bind(crate::own_addr(), port, socket_set)?;

        let addr = to_m3_addr(crate::own_addr());
        reply_vmsg!(is, Code::None as i32, addr.0)
    }

    pub fn listen(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: Sd = is.pop()?;
        let port: Port = is.pop()?;

        log!(
            crate::LOG_SESS,
            "[{}] net::listen(sd={}, port={})",
            self.server_session.ident(),
            sd,
            port
        );

        if !self.can_use_port(port) {
            return Err(Error::new(Code::NoPerm));
        }

        let socket = self.get_socket(sd)?;
        socket
            .borrow_mut()
            .listen(socket_set, crate::own_addr(), port)?;

        let addr = to_m3_addr(crate::own_addr());
        reply_vmsg!(is, Code::None as i32, addr.0)
    }

    pub fn connect(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: Sd = is.pop()?;
        let remote_addr = IpAddr(is.pop::<u32>()?);
        let remote_port: Port = is.pop()?;

        let local_port = ports::alloc();
        log!(
            crate::LOG_SESS,
            "[{}] net::connect(sd={}, remote={}:{}, local={})",
            self.server_session.ident(),
            sd,
            remote_addr,
            remote_port,
            local_port
        );

        let sock = self.get_socket(sd)?;
        let port_no = *local_port;
        sock.borrow_mut()
            .connect(remote_addr, remote_port, local_port, socket_set)?;

        let addr = to_m3_addr(crate::own_addr());
        reply_vmsg!(is, Code::None as i32, addr.0, port_no)
    }

    pub fn abort(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        let sd: Sd = is.pop()?;
        let remove: bool = is.pop()?;

        log!(
            crate::LOG_SESS,
            "[{}] net::abort(sd={}, remove={})",
            self.server_session.ident(),
            sd,
            remove
        );

        let socket = self.get_socket(sd)?;
        socket.borrow_mut().abort(socket_set);
        if remove {
            self.remove_socket(sd);
        }
        is.reply_error(Code::None)
    }

    pub fn process_incoming(&mut self, socket_set: &mut SocketSet<'static>) -> bool {
        let sess = self.server_session.ident();
        let mut queued_events = false;

        // iterate over all sockets and check for events
        'outer_loop: for idx in 0..self.sockets.len() {
            if let Some(socket) = self.sockets.get(idx).unwrap() {
                let mut sock = socket.borrow_mut();
                let chan = sock.channel().clone();

                chan.fetch_replies();

                if !sock.process_queued_events(sess, socket_set) {
                    queued_events = true;
                    continue 'outer_loop;
                }

                // receive everything in the channel
                while let Some(event) = chan.receive_event() {
                    if !sock.process_event(sess, socket_set, event) {
                        queued_events = true;
                        continue 'outer_loop;
                    }
                }
            }
        }

        queued_events
    }

    pub fn process_outgoing(&mut self, socket_set: &mut SocketSet<'static>) {
        // iterate over all sockets and try to receive
        for socket in self.sockets.iter().flatten() {
            let socket_sd = socket.borrow().sd();
            let chan = socket.borrow().channel().clone();

            chan.fetch_replies();

            // if we don't have credits anymore to send events, stop here. we'll get a reply
            // to one of our earlier events and get credits back with this, so that we'll wake
            // up from a potential sleep and call receive again.
            if !chan.can_send().unwrap() {
                break;
            }

            if let Some(event) = socket.borrow_mut().fetch_event(socket_set) {
                log!(
                    crate::LOG_DATA,
                    "[{}] socket {}: received event {:?}",
                    socket_sd,
                    self.server_session.ident(),
                    event,
                );

                // the match is needed, because we don't want to send the enum, but the
                // contained event struct
                match event {
                    SendNetEvent::Connected(e) => chan.send_event(e).unwrap(),
                    SendNetEvent::Closed(e) => chan.send_event(e).unwrap(),
                    SendNetEvent::CloseReq(e) => chan.send_event(e).unwrap(),
                }
            }

            if !chan.can_send().unwrap() {
                break;
            }

            socket.borrow_mut().receive(socket_set, |data, addr| {
                let ep = to_m3_ep(addr);
                let amount = cmp::min(event::MTU, data.len());

                log!(
                    crate::LOG_DATA,
                    "[{}] socket {}: received packet with {}b from {}",
                    socket_sd,
                    self.server_session.ident(),
                    amount,
                    ep
                );

                chan.send_data(ep, amount, |buf| {
                    buf[0..amount].copy_from_slice(&data[0..amount]);
                })
                .unwrap();
                amount
            });
        }
    }
}
