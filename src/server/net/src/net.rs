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
#![feature(duration_constants)]
#![no_std]

use core::str::FromStr;

use m3::cap::Selector;
use m3::cell::LazyStaticCell;
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, RecvGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::math;
use m3::pes::VPE;
use m3::rc::Rc;
use m3::server::{CapExchange, Handler, Server, SessId, SessionContainer, DEF_MAX_CLIENTS};
use m3::session::NetworkOp;
use m3::time::{TimeDuration, TimeInstant};
use m3::{log, println};

use smoltcp::iface::{InterfaceBuilder, NeighborCache};
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

static OWN_IP: LazyStaticCell<IpAddress> = LazyStaticCell::default();
static OWN_MAC: [u8; 6] = [0x00, 0x0A, 0x35, 0x03, 0x02, 0x03];

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
                NetworkOp::BIND => sess.bind(is, &mut self.socket_set),
                NetworkOp::LISTEN => sess.listen(is, &mut self.socket_set),
                NetworkOp::CONNECT => sess.connect(is, &mut self.socket_set),
                NetworkOp::ABORT => sess.abort(is, &mut self.socket_set),
                NetworkOp::GET_IP => sess.get_ip(is),
                _ => Err(Error::new(Code::InvArgs)),
            }
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    // processes outgoing events to clients
    fn process_outgoing(&mut self) -> bool {
        let socks = &mut self.socket_set;
        let mut res = false;
        self.sessions.for_each(|s| {
            if let NetworkSession::SocketSession(ss) = s {
                res |= ss.process_outgoing(socks)
            }
        });
        res
    }

    // processes incoming events from clients and returns whether there is still work to do
    fn process_incoming(&mut self) -> bool {
        let socks = &mut self.socket_set;
        let mut res = false;
        self.sessions.for_each(|s| {
            if let NetworkSession::SocketSession(ss) = s {
                res |= ss.process_incoming(socks)
            }
        });
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

        self.sessions.add_next(crt, srv_sel, false, |sess| {
            log!(LOG_SESS, "[{}] net::open(sel={})", sess.ident(), sess.sel());
            Ok(NetworkSession::SocketSession(sess::SocketSession::new(
                crt, arg, sess, rgate,
            )?))
        })
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
            s.obtain(crt, self.sel, xchg, &mut self.socket_set)
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
        log!(LOG_DEF, "Shutdown request");
    }
}

pub fn own_ip() -> IpAddress {
    OWN_IP.get()
}

#[derive(Clone, Debug)]
pub struct NetSettings {
    driver: String,
    name: String,
    ip: smoltcp::wire::Ipv4Address,
    max_clients: usize,
}

impl Default for NetSettings {
    fn default() -> Self {
        NetSettings {
            driver: String::from("default"),
            name: String::default(),
            ip: smoltcp::wire::Ipv4Address::default(),
            max_clients: DEF_MAX_CLIENTS,
        }
    }
}

fn usage() -> ! {
    println!(
        "Usage: {} [-d <driver>] [-m <max-clients>] <name> <ip>",
        env::args().next().unwrap()
    );
    println!();
    println!("  -d: the driver to use (lo=loopback or default=E1000/Fifo)");
    println!("  -m: the maximum number of clients (receive slots)");
    m3::exit(1);
}

fn parse_args() -> Result<NetSettings, String> {
    let mut settings = NetSettings::default();

    let args: Vec<&str> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i] {
            "-m" => {
                settings.max_clients = args[i + 1]
                    .parse::<usize>()
                    .map_err(|_| String::from("Failed to parse client count"))?;
                i += 1;
            },
            "-d" => {
                settings.driver = args[i + 1].to_string();
                i += 1;
            },
            _ => break,
        }
        i += 1;
    }

    if i == args.len() {
        usage();
    }

    settings.name = args.get(i).expect("Failed to read name!").to_string();
    settings.ip =
        smoltcp::wire::Ipv4Address::from_str(args.get(i + 1).expect("Failed to read ip!"))
            .expect("Failed to convert IP address!");
    Ok(settings)
}

#[no_mangle]
pub fn main() -> i32 {
    smoltcpif::logger::init().unwrap();

    let settings = parse_args().unwrap_or_else(|e| {
        println!("Invalid arguments: {}", e);
        usage();
    });

    let mut rgate = RecvGate::new(
        math::next_log2(sess::MSG_SIZE * settings.max_clients),
        math::next_log2(sess::MSG_SIZE),
    )
    .expect("failed to create main rgate for handler!");

    rgate.activate().expect("Failed to activate main rgate");

    let mut neighbor_cache_entries = [None; 8];
    let neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);

    let ip_addr = IpCidr::new(IpAddress::Ipv4(settings.ip), 8);
    OWN_IP.set(ip_addr.address());
    ports::init(MAX_SOCKETS);

    let mut iface = if settings.driver == "lo" {
        driver::DriverInterface::Lo(
            InterfaceBuilder::new(smoltcp::phy::Loopback::new(smoltcp::phy::Medium::Ethernet))
                .ethernet_addr(EthernetAddress::from_bytes(&OWN_MAC))
                .neighbor_cache(neighbor_cache)
                .ip_addrs([ip_addr])
                .finalize(),
        )
    }
    else {
        #[cfg(target_vendor = "gem5")]
        let device = driver::E1000Device::new().expect("Failed to create E1000 driver");
        #[cfg(target_vendor = "hw")]
        let device = driver::AXIEthDevice::new().expect("Failed to create AXI ethernet driver");
        #[cfg(target_vendor = "host")]
        let device = driver::DevFifo::new(&settings.name);
        driver::DriverInterface::Eth(
            InterfaceBuilder::new(device)
                .ethernet_addr(EthernetAddress::from_bytes(&OWN_MAC))
                .neighbor_cache(neighbor_cache)
                .ip_addrs([ip_addr])
                .finalize(),
        )
    };

    let socket_set = SocketSet::new(Vec::with_capacity(MAX_SOCKETS));

    let mut handler = NetHandler {
        sel: 0,
        sessions: SessionContainer::new(settings.max_clients),
        socket_set,
        rgate: Rc::new(rgate),
    };

    let serv = Server::new(&settings.name, &mut handler).expect("Failed to create server!");
    handler.sel = serv.sel();

    log!(
        LOG_DEF,
        "netrs: created service {} with ip={} and driver={}",
        settings.name,
        settings.ip,
        settings.driver
    );

    let rgatec = handler.rgate.clone();
    let start = TimeInstant::now();

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

            let cur_time = smoltcp::time::Instant::from_millis(start.elapsed().as_millis() as i64);

            // now poll smoltcp to send and receive packets
            if let Err(e) = iface.poll(&mut handler.socket_set, cur_time) {
                log!(LOG_DETAIL, "netrs: poll failed: {}", e);
            }

            // check for outgoing events we have to send to clients
            let recvs_pending = handler.process_outgoing();

            if !sends_pending && !recvs_pending {
                // ask smoltcp how long we can sleep
                match iface.poll_delay(&handler.socket_set, cur_time) {
                    // we need to call it again immediately => continue the loop
                    Some(Duration { millis: 0 }) => continue,
                    // we should not wait longer than `n` => sleep for `n`
                    Some(n) => break TimeDuration::from_millis(n.total_millis()),
                    // smoltcp has nothing to do => sleep until the next TCU message arrives
                    None => break TimeDuration::MAX,
                }
            }
        };

        log!(LOG_DETAIL, "Sleeping for {:?}", sleep_nanos);
        VPE::sleep_for(sleep_nanos).ok();
    }

    0
}
