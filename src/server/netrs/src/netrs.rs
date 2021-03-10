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

// for offset_of with unstable_const feature
#![feature(const_maybe_uninit_as_ptr)]
#![feature(const_raw_ptr_deref)]
#![feature(const_ptr_offset_from)]
#![no_std]

use core::str::FromStr;

use m3::cap::Selector;
use m3::cell::RefCell;
use m3::col::Vec;
use m3::com::{GateIStream, RecvGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::math;
use m3::rc::Rc;
use m3::server::{CapExchange, Handler, Server, SessId, SessionContainer};
use m3::session::NetworkOp;
use m3::{log, println, reply_vmsg};

//Smol tcp network stuff
use smoltcp::iface::{EthernetInterfaceBuilder, NeighborCache};
use smoltcp::socket::SocketSet;
use smoltcp::time::Duration;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};

use crate::sess::socket_session::MAX_SOCKETS;
use crate::sess::NetworkSession;

pub mod driver;
pub mod sess;
mod smoltcplogger;
pub mod util;

pub const LOG_DEF: bool = false;
pub const LOG_NIC: bool = false;
pub const LOG_SMOLTCP: bool = false;

struct NetHandler {
    sel: Selector,
    sessions: SessionContainer<NetworkSession>,
    ///Holds all the actual smoltcp sockets. Used for polling events on them.
    socket_set: SocketSet<'static>,
    rgate: Rc<RefCell<RecvGate>>,
    ///True if shutdown was called.
    shuting_down: bool,
}

impl NetHandler {
    fn handle(&mut self, op: NetworkOp, is: &mut GateIStream) -> Result<(), Error> {
        log!(
            LOG_DEF,
            "net::handle(net_op={:?}, session={})",
            op,
            is.label() as SessId
        );

        let sess_id: SessId = is.label() as SessId;

        if let Some(sess) = self.sessions.get_mut(sess_id) {
            match op {
                NetworkOp::STAT => sess.stat(is),
                NetworkOp::SEEK => sess.seek(is),
                NetworkOp::NEXT_IN => sess.next_in(is),
                NetworkOp::NEXT_OUT => sess.next_out(is),
                NetworkOp::COMMIT => sess.commit(is),
                NetworkOp::CLOSE => sess.close(is, &mut self.socket_set),
                NetworkOp::CREATE => sess.create(is, &mut self.socket_set),
                NetworkOp::BIND => sess.bind(is, &mut self.socket_set),
                NetworkOp::LISTEN => sess.listen(is, &mut self.socket_set),
                NetworkOp::CONNECT => sess.connect(is, &mut self.socket_set),
                NetworkOp::ACCEPT => sess.accept(is, &mut self.socket_set),
                NetworkOp::COUNT => sess.count(is, &mut self.socket_set),
                NetworkOp::QUERY_STATE => sess.query_state(is, &mut self.socket_set),
                NetworkOp::TICK => reply_vmsg!(is, Code::None as i32), //a tick does nothing, but lets the smoltcp stack do its work
                _ => {
                    log!(LOG_DEF, "Net::handle got invalid NetworkOp: {}", op);
                    Err(Error::new(Code::InvArgs))
                },
            }
        }
        else {
            log!(LOG_DEF, "No session found with label/id={}", sess_id);
            Err(Error::new(Code::InvArgs))
        }
    }

    fn tick_receive(&mut self) {
        for i in 0..self.sessions.capacity() {
            if let Some(sess) = self.sessions.get_mut(i) {
                match sess {
                    NetworkSession::SocketSession(ss) => ss.receive(&mut self.socket_set),
                    _ => {},
                }
            }
        }
    }

    //Checks each socket session if it should send data. If so, queues the send on the socket.
    fn tick_send(&mut self) {
        for i in 0..self.sessions.capacity() {
            if let Some(sess) = self.sessions.get_mut(i) {
                match sess {
                    NetworkSession::SocketSession(ss) => ss.send(&mut self.socket_set),
                    _ => {},
                }
            }
        }
    }
}

impl Handler<NetworkSession> for NetHandler {
    fn sessions(&mut self) -> &mut SessionContainer<NetworkSession> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        _arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        //Needed to satisfy the borrow checker
        let rgate = self.rgate.clone();

        let res = self.sessions.add_next(crt, srv_sel, false, |sess| {
            log!(LOG_DEF, "[{}] net::open(sel={})", sess.ident(), sess.sel());
            let new_session = NetworkSession::SocketSession(sess::SocketSession::new(
                crt,
                sess,
                rgate.clone(), //clone also needed to satisfy the borrow checker, otherwise E0507 occurred.
            ));
            Ok(new_session)
        });

        assert!(res.is_ok());
        res
    }

    fn obtain(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        log!(crate::LOG_DEF, "netrs::obtain(crt={}, sid={})", crt, sid);

        if let Some(s) = self.sessions.get_mut(sid) {
            //If this is a socket session. Create a send gate, that can be used to communicate with this
            //request handler.

            let res = s.obtain(crt, self.sel, xchg);
            log!(crate::LOG_DEF, "End obtain");
            res
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    fn delegate(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        log!(crate::LOG_DEF, "netrs::delegate(crt={}, sid={})", crt, sid);
        if let Some(s) = self.sessions.get_mut(sid) {
            s.delegate(xchg)
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        self.sessions.remove(crt, sid);
    }

    fn shutdown(&mut self) {
        log!(LOG_DEF, "NetRs: Shutdown");
        self.shuting_down = true;
        /*
        TODO:
        Drop each session.
        driver stop?
        rgate stop
         */
    }
}

/// Executes the server loop, calling `func` in every iteration.
pub fn my_server_loop<F: FnMut() -> Result<(), Error>>(mut func: F) -> Result<(), Error> {
    loop {
        func()?;
    }
}

#[no_mangle]
pub fn main() -> i32 {
    //Parse args
    let args: Vec<&str> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: {} <name> <ip address> <netmask>", args[0]);
        return -1;
    }

    let name = args.get(1).expect("Failed to read name!");

    smoltcplogger::init().unwrap();

    let ip = smoltcp::wire::Ipv4Address::from_str(args.get(2).expect("Failed to read ip!"))
        .expect("Failed to convert IP address!");
    let netmask =
        smoltcp::wire::Ipv4Address::from_str(args.get(3).expect("Failed to read netmask!"))
            .expect("Failed to create netmask!");

    let mut rgate = if let Ok(rg) = RecvGate::new(
        math::next_log2(sess::MSG_SIZE * 32),
        math::next_log2(sess::MSG_SIZE),
    ) {
        rg
    }
    else {
        log!(LOG_DEF, "failed to create main rgate for handler!");
        return -1;
    };

    rgate.activate().expect("Failed to activate main rgate");

    //Create interface to networking device
    //Depending on the platform, create a networking device.
    #[cfg(target_os = "none")]
    let device = driver::driver::E1000Device::new().expect("Failed to create E1000 driver");

    #[cfg(target_os = "linux")]
    let device = driver::driver::DevFifo::new(name).expect("Failed to create FIFO Driver");

    let mut neighbor_cache_entries = [None; 8];
    let neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);

    let ip_addrs = [IpCidr::new(IpAddress::Ipv4(ip), 8)];
    let mut iface = EthernetInterfaceBuilder::new(device)
        .ethernet_addr(EthernetAddress::default())
        .neighbor_cache(neighbor_cache)
        .ip_addrs(ip_addrs)
        .finalize();

    let socket_set = SocketSet::new(Vec::with_capacity(MAX_SOCKETS));

    let mut handler = NetHandler {
        sel: 0,
        sessions: SessionContainer::new(m3::server::DEF_MAX_CLIENTS),
        socket_set,
        rgate: Rc::new(RefCell::new(rgate)),
        shuting_down: false,
    };

    let serv = Server::new(name, &mut handler).expect("Failed to create server!");
    handler.sel = serv.sel();

    log!(
        LOG_DEF,
        "Created name={}, ip={}, netmask={}",
        name,
        ip,
        netmask
    );

    let rgatec = handler.rgate.clone();

    log!(LOG_DEF, "Started net server");

    let mut clock = smoltcp::time::Instant::from_millis(0);

    my_server_loop(|| {
        //log!(crate::LOG_DEF, "POLL");
        serv.handle_ctrl_chan(&mut handler)?;
        {
            let rgate = rgatec.borrow();
            //Check if we got some messages through our main rgate.
            if let Some(msg) = rgate.fetch() {
                let mut is = GateIStream::new(msg, &rgate);
                let op = is.pop::<NetworkOp>()?;
                if let Err(e) = handler.handle(op, &mut is) {
                    is.reply_error(e.code()).ok();
                }
            }
        }

        //Tick all socket sessions to receive packages that are in the tcp socket.
        handler.tick_receive();
        //Tick all socket sessions to send data thats on the channel to the socket.
        handler.tick_send();

        //log!(crate::LOG_DEF, "Poll");
        match iface.poll(&mut handler.socket_set, clock) {
            Ok(_) => {},
            Err(_e) => {
                //Not loging any error since those happen fairly often.
                //TODO match error and lock the important ones.
                //log!(LOG_DEF, "Poll error: {}", e);
            },
        }
        //log!(crate::LOG_DEF, "Poll Delay");
        match iface.poll_delay(&handler.socket_set, clock) {
            Some(Duration { millis: 0 }) => clock += Duration::from_millis(1),
            Some(delay) => {
                //log!(LOG_DEF, "sleeping for {} ms", delay);
                clock += delay;
            },
            None => clock += Duration::from_millis(1),
        }

        Ok(())
    })
    .ok();

    log!(crate::LOG_DEF, "SERVER ENDED");
    0
}
