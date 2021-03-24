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

// for offset_of with unstable_const feature
#![feature(const_maybe_uninit_as_ptr)]
#![feature(const_raw_ptr_deref)]
#![feature(const_ptr_offset_from)]
#![no_std]

use core::str::FromStr;

use m3::cap::Selector;
use m3::cell::LazyStaticCell;
use m3::col::Vec;
use m3::com::{GateIStream, RecvGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::math;
use m3::rc::Rc;
use m3::server::{CapExchange, Handler, Server, SessId, SessionContainer};
use m3::session::NetworkOp;
use m3::tcu::TCU;
use m3::{log, println};

use smoltcp::iface::{EthernetInterfaceBuilder, NeighborCache};
use smoltcp::socket::SocketSet;
use smoltcp::time::Duration;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};

use crate::sess::NetworkSession;

mod driver;
mod ports;
mod sess;
mod smoltcpif;

pub const LOG_ERR: bool = true;
pub const LOG_DEF: bool = true;
pub const LOG_SESS: bool = false;
pub const LOG_DATA: bool = false;
pub const LOG_PORTS: bool = false;
pub const LOG_NIC: bool = false;
pub const LOG_NIC_DETAIL: bool = false;
pub const LOG_SMOLTCP: bool = false;
pub const LOG_DETAIL: bool = false;

const MAX_SOCKETS: usize = 64;

static OWN_ADDR: LazyStaticCell<IpAddress> = LazyStaticCell::default();

struct NetHandler {
    // our service selector
    sel: Selector,
    // our sessions
    sessions: SessionContainer<NetworkSession>,
    // holds all the actual smoltcp sockets. Used for polling events on them.
    socket_set: SocketSet<'static>,
    // the receive gates for requests from clients
    rgate: Rc<RecvGate>,
}

impl NetHandler {
    fn handle(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        let op = is.pop::<NetworkOp>()?;
        let sess_id: SessId = is.label() as SessId;

        if let Some(sess) = self.sessions.get_mut(sess_id) {
            match op {
                NetworkOp::STAT => sess.stat(is),
                NetworkOp::SEEK => sess.seek(is),
                NetworkOp::NEXT_IN => sess.next_in(is),
                NetworkOp::NEXT_OUT => sess.next_out(is),
                NetworkOp::COMMIT => sess.commit(is),
                NetworkOp::CREATE => sess.create(is, &mut self.socket_set),
                NetworkOp::BIND => sess.bind(is, &mut self.socket_set),
                NetworkOp::LISTEN => sess.listen(is, &mut self.socket_set),
                NetworkOp::CONNECT => sess.connect(is, &mut self.socket_set),
                NetworkOp::ABORT => sess.abort(is, &mut self.socket_set),
                _ => Err(Error::new(Code::InvArgs)),
            }
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    // processes outgoing events to clients
    fn process_outgoing(&mut self) {
        for i in 0..self.sessions.capacity() {
            if let Some(sess) = self.sessions.get_mut(i) {
                match sess {
                    NetworkSession::SocketSession(ss) => ss.process_outgoing(&mut self.socket_set),
                    _ => {},
                }
            }
        }
    }

    // processes incoming events from clients and returns whether there is still work to do
    fn process_incoming(&mut self) -> bool {
        let mut res = false;
        for i in 0..self.sessions.capacity() {
            if let Some(sess) = self.sessions.get_mut(i) {
                match sess {
                    NetworkSession::SocketSession(ss) => {
                        res |= ss.process_incoming(&mut self.socket_set)
                    },
                    _ => {},
                }
            }
        }
        res
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
        arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        let rgate = self.rgate.clone();

        let res = self.sessions.add_next(crt, srv_sel, false, |sess| {
            log!(LOG_SESS, "[{}] net::open(sel={})", sess.ident(), sess.sel());
            Ok(NetworkSession::SocketSession(sess::SocketSession::new(
                crt, arg, sess, rgate,
            )?))
        });

        assert!(res.is_ok());
        res
    }

    fn obtain(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        log!(
            LOG_SESS,
            "[{}] net::obtain(crt={}, #caps={})",
            sid,
            crt,
            xchg.in_caps()
        );

        if let Some(s) = self.sessions.get_mut(sid) {
            s.obtain(crt, self.sel, xchg)
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    fn delegate(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        log!(LOG_SESS, "[{}] net::delegate(crt={})", sid, crt);

        if let Some(s) = self.sessions.get_mut(sid) {
            s.delegate(xchg)
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        log!(LOG_SESS, "[{}] net::close(crt={})", sid, crt);

        self.sessions.remove(crt, sid);
    }

    fn shutdown(&mut self) {
        log!(LOG_DEF, "Shutting down");
    }
}

pub fn own_addr() -> IpAddress {
    *OWN_ADDR
}

#[no_mangle]
pub fn main() -> i32 {
    let args: Vec<&str> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: {} <name> <ip address> <netmask>", args[0]);
        return -1;
    }

    let name = args.get(1).expect("Failed to read name!");

    smoltcpif::logger::init().unwrap();

    let ip = smoltcp::wire::Ipv4Address::from_str(args.get(2).expect("Failed to read ip!"))
        .expect("Failed to convert IP address!");
    let netmask =
        smoltcp::wire::Ipv4Address::from_str(args.get(3).expect("Failed to read netmask!"))
            .expect("Failed to create netmask!");

    let mut rgate = RecvGate::new(
        math::next_log2(sess::MSG_SIZE * 32),
        math::next_log2(sess::MSG_SIZE),
    )
    .expect("failed to create main rgate for handler!");

    rgate.activate().expect("Failed to activate main rgate");

    #[cfg(target_os = "none")]
    let device = driver::driver::E1000Device::new().expect("Failed to create E1000 driver");
    #[cfg(target_os = "linux")]
    let device = driver::driver::DevFifo::new(name);

    let mut neighbor_cache_entries = [None; 8];
    let neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);

    let ip_addr = IpCidr::new(IpAddress::Ipv4(ip), 8);
    OWN_ADDR.set(ip_addr.address());
    ports::init(MAX_SOCKETS);

    let mut iface = EthernetInterfaceBuilder::new(device)
        .ethernet_addr(EthernetAddress::default())
        .neighbor_cache(neighbor_cache)
        .ip_addrs([ip_addr])
        .finalize();

    let socket_set = SocketSet::new(Vec::with_capacity(MAX_SOCKETS));

    let mut handler = NetHandler {
        sel: 0,
        sessions: SessionContainer::new(m3::server::DEF_MAX_CLIENTS),
        socket_set,
        rgate: Rc::new(rgate),
    };

    let serv = Server::new(name, &mut handler).expect("Failed to create server!");
    handler.sel = serv.sel();

    log!(
        LOG_DEF,
        "netrs: created service {} with ip={} and netmask={}",
        name,
        ip,
        netmask
    );

    let rgatec = handler.rgate.clone();

    'outer: loop {
        let sleep_nanos = loop {
            if serv.handle_ctrl_chan(&mut handler).is_err() {
                break 'outer;
            }

            // Check if we got some messages through our main rgate.
            if let Some(msg) = rgatec.fetch() {
                let mut is = GateIStream::new(msg, &rgatec);
                if let Err(e) = handler.handle(&mut is) {
                    is.reply_error(e.code()).ok();
                }
            }

            // receive events from clients and push data to send into smoltcp sockets
            let sends_pending = handler.process_incoming();

            let cur_time = smoltcp::time::Instant::from_millis(TCU::nanotime() as i64 / 1_000_000);

            // now poll smoltcp to send and receive packets
            if let Err(e) = iface.poll(&mut handler.socket_set, cur_time) {
                log!(LOG_DETAIL, "netrs: poll failed: {}", e);
            }

            // check for outgoing events we have to send to clients
            handler.process_outgoing();

            if !sends_pending {
                // ask smoltcp how long we can sleep
                match iface.poll_delay(&handler.socket_set, cur_time) {
                    // we need to call it again immediately => continue the loop
                    Some(Duration { millis: 0 }) => continue,
                    // we should not wait longer than `n` => sleep for `n`
                    Some(n) => break n.total_millis() as u64 * 1_000_000,
                    // smoltcp has nothing to do => sleep until the next TCU message arrives
                    None => break 0,
                }
            }
        };

        log!(LOG_DETAIL, "Sleeping for {} ns", sleep_nanos);
        m3::pes::VPE::sleep_for(sleep_nanos).ok();
    }

    0
}
