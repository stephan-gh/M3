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

#![no_std]

use core::str::FromStr;

use m3::cap::Selector;
use m3::cell::{LazyStaticCell, StaticRefCell};
use m3::col::{BTreeMap, String, ToString, Vec};
use m3::com::{opcodes, GateIStream};
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::net::{log_net, NetLogEvent};
use m3::server::{
    CapExchange, ExcType, Handler, RequestHandler, Server, SessId, SessionContainer,
    DEF_MAX_CLIENTS,
};
use m3::tiles::OwnActivity;
use m3::time::{TimeDuration, TimeInstant};
use m3::{env, reply_vmsg};
use m3::{log, println};

use smoltcp::iface::{InterfaceBuilder, NeighborCache, Routes, SocketHandle};
use smoltcp::wire::{EthernetAddress, IpAddress, Ipv4Cidr};

use crate::driver::DriverInterface;
use crate::sess::SocketSession;
use crate::smoltcpif::socket::to_m3_addr;

mod driver;
mod ports;
mod sess;
mod smoltcpif;

const MAX_SOCKETS: usize = 64;
const MSG_SIZE: usize = 128;

static OWN_IP: LazyStaticCell<IpAddress> = LazyStaticCell::default();
static NAMESERVER: LazyStaticCell<IpAddress> = LazyStaticCell::default();
static OWN_MAC: [u8; 6] = [0x00, 0x0A, 0x35, 0x03, 0x02, 0x03];
static TIMEOUTS: StaticRefCell<Vec<(SocketHandle, TimeInstant)>> = StaticRefCell::new(Vec::new());

pub fn add_timeout(handle: SocketHandle, timeout: TimeInstant) {
    TIMEOUTS.borrow_mut().push((handle, timeout));
}

pub fn remove_timeout(handle: SocketHandle) {
    TIMEOUTS.borrow_mut().retain(|t| t.0 != handle);
}

fn next_timeout() -> Option<TimeInstant> {
    TIMEOUTS
        .borrow()
        .iter()
        .min_by(|a, b| a.1.cmp(&b.1))
        .map(|t| t.1)
}

struct NetHandler<'a> {
    reqhdl: RequestHandler<SocketSession, opcodes::Net>,
    // holds all the actual smoltcp sockets. Used for polling events on them.
    iface: DriverInterface<'a>,
}

impl Handler<SocketSession> for NetHandler<'_> {
    fn sessions(&mut self) -> &mut SessionContainer<SocketSession> {
        self.reqhdl.sessions()
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        self.reqhdl.open(crt, srv_sel, arg)
    }

    fn exchange(
        &mut self,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
        obtain: bool,
    ) -> Result<(), Error> {
        let Self { reqhdl, iface } = self;

        reqhdl.handle_capxchg_with(crt, sid, xchg, obtain, |reqhdl, opcode, ty, xchg| {
            assert!(opcode == opcodes::Net::Create.into());

            let sess = reqhdl
                .clients_mut()
                .get_mut(sid)
                .ok_or_else(|| Error::new(Code::InvArgs))?;
            match ty {
                ExcType::Obt(_) => sess.create_socket(xchg, iface),
                ExcType::Del(_) => Err(Error::new(Code::InvArgs)),
            }
        })
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        if let Some(s) = self.reqhdl.clients_mut().get_mut(sid) {
            s.abort_all(&mut self.iface).unwrap();
        }

        self.reqhdl.clients_mut().remove(crt, sid);
    }
}

impl NetHandler<'_> {
    fn fetch_and_handle(&mut self) {
        let Self { reqhdl, iface } = self;

        reqhdl.fetch_and_handle_msg_with(|_, opcode, sess, is| match opcode {
            o if o == opcodes::Net::Bind.into() => sess.bind(is, iface),
            o if o == opcodes::Net::Listen.into() => sess.listen(is, iface),
            o if o == opcodes::Net::Connect.into() => sess.connect(is, iface),
            o if o == opcodes::Net::Abort.into() => sess.abort(is, iface),
            o if o == opcodes::Net::GetIP.into() => Self::get_ip(is),
            o if o == opcodes::Net::GetNameSrv.into() => Self::get_nameserver(is),
            _ => Err(Error::new(Code::InvArgs)),
        });
    }

    fn get_ip(is: &mut GateIStream<'_>) -> Result<(), Error> {
        let addr = to_m3_addr(OWN_IP.get());
        reply_vmsg!(is, Code::Success, addr.0)
    }

    fn get_nameserver(is: &mut GateIStream<'_>) -> Result<(), Error> {
        if !NAMESERVER.is_some() {
            return Err(Error::new(Code::NotSup));
        }

        let addr = to_m3_addr(NAMESERVER.get());
        reply_vmsg!(is, Code::Success, addr.0)
    }

    // processes outgoing events to clients
    fn process_outgoing(&mut self) -> bool {
        let iface = &mut self.iface;
        let mut res = false;
        self.reqhdl.clients_mut().for_each(|s| {
            res |= s.process_outgoing(iface);
        });
        res
    }

    // processes incoming events from clients and returns whether there is still work to do
    fn process_incoming(&mut self) -> bool {
        let iface = &mut self.iface;
        let mut res = false;
        self.reqhdl.clients_mut().for_each(|s| {
            res |= s.process_incoming(iface);
        });
        res
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
    netmask: smoltcp::wire::Ipv4Address,
    nameserver: Option<smoltcp::wire::Ipv4Address>,
    gateway: Option<smoltcp::wire::Ipv4Address>,
    max_clients: usize,
}

impl Default for NetSettings {
    fn default() -> Self {
        NetSettings {
            driver: String::from("default"),
            name: String::default(),
            netmask: smoltcp::wire::Ipv4Address::new(255, 255, 255, 0),
            ip: smoltcp::wire::Ipv4Address::default(),
            nameserver: None,
            gateway: None,
            max_clients: DEF_MAX_CLIENTS,
        }
    }
}

fn usage() -> ! {
    println!(
        "Usage: {} [-d <driver>] [-m <max-clients>] [-a <netmask>] [-n <nameserver>] [-g <gateway>] <name> <ip>",
        env::args().next().unwrap()
    );
    println!();
    println!("  -d: the driver to use (lo=loopback or default=E1000/Fifo)");
    println!("  -m: the maximum number of clients (receive slots)");
    println!("  -a: the network mask to use (default: 255.255.255.0)");
    println!("  -n: the IP address of the DNS server");
    println!("  -g: the IP address of the default gateway");
    OwnActivity::exit_with(Code::InvArgs);
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
            "-a" => {
                settings.netmask = smoltcp::wire::Ipv4Address::from_str(
                    args.get(i + 1).expect("Failed to read netmask!"),
                )
                .expect("Failed to parse netmask!");
                i += 1;
            },
            "-n" => {
                settings.nameserver = Some(
                    smoltcp::wire::Ipv4Address::from_str(
                        args.get(i + 1).expect("Failed to read nameserver!"),
                    )
                    .expect("Failed to parse nameserver IP!"),
                );
                i += 1;
            },
            "-g" => {
                settings.gateway = Some(
                    smoltcp::wire::Ipv4Address::from_str(
                        args.get(i + 1).expect("Failed to read gateway!"),
                    )
                    .expect("Failed to parse gateway IP!"),
                );
                i += 1;
            },
            _ => break,
        }
        i += 1;
    }

    if args.len() < i + 2 {
        usage();
    }

    settings.name = args.get(i).expect("Failed to read name!").to_string();
    settings.ip =
        smoltcp::wire::Ipv4Address::from_str(args.get(i + 1).expect("Failed to read ip!"))
            .expect("Failed to parse IP address!");
    Ok(settings)
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    smoltcpif::logger::init().unwrap();

    let settings = parse_args().unwrap_or_else(|e| {
        println!("Invalid arguments: {}", e);
        usage();
    });

    let mut neighbor_cache_entries = [None; 8];
    let neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);

    let ip_cidr = smoltcp::wire::IpCidr::Ipv4(
        Ipv4Cidr::from_netmask(settings.ip, settings.netmask)
            .expect("Invalid IP-address/netmask pair"),
    );
    let ip_addr = ip_cidr.address();
    OWN_IP.set(ip_addr);

    if let Some(ns) = settings.nameserver {
        let ns_cidr =
            Ipv4Cidr::from_netmask(ns, settings.netmask).expect("Invalid nameserver/netmask pair");
        NAMESERVER.set(IpAddress::Ipv4(ns_cidr.address()));
    }

    let mut routes = Routes::new(BTreeMap::new());
    if let Some(gw) = settings.gateway {
        routes
            .add_default_ipv4_route(gw)
            .expect("Cannot add default route");
    }

    ports::init(MAX_SOCKETS);

    let iface = if settings.driver == "lo" {
        driver::DriverInterface::Lo(
            InterfaceBuilder::new(
                smoltcp::phy::Loopback::new(smoltcp::phy::Medium::Ethernet),
                Vec::with_capacity(MAX_SOCKETS),
            )
            .hardware_addr(EthernetAddress::from_bytes(&OWN_MAC).into())
            .neighbor_cache(neighbor_cache)
            .ip_addrs([ip_cidr])
            .routes(routes)
            .finalize(),
        )
    }
    else {
        #[cfg(target_vendor = "gem5")]
        let device = driver::E1000Device::new().expect("Failed to create E1000 driver");
        #[cfg(any(target_vendor = "hw", target_vendor = "hw22"))]
        let device = driver::AXIEthDevice::new().expect("Failed to create AXI ethernet driver");
        driver::DriverInterface::Eth(
            InterfaceBuilder::new(device, Vec::with_capacity(MAX_SOCKETS))
                .hardware_addr(EthernetAddress::from_bytes(&OWN_MAC).into())
                .neighbor_cache(neighbor_cache)
                .ip_addrs([ip_cidr])
                .routes(routes)
                .finalize(),
        )
    };

    let mut handler = NetHandler {
        reqhdl: RequestHandler::new_with(settings.max_clients, MSG_SIZE, 1)
            .expect("Unable to create request handler"),
        iface,
    };

    let serv = Server::new(&settings.name, &mut handler).expect("Failed to create server!");

    log!(
        LogFlags::Info,
        concat!(
            "netrs: created service {} with {{\n",
            "  driver={},\n",
            "  ip={:?},\n",
            "  nameserver={:?},\n",
            "  gateway={:?},\n",
            "}}"
        ),
        settings.name,
        settings.driver,
        settings.ip,
        settings.nameserver,
        settings.gateway,
    );

    let start = TimeInstant::now();

    'outer: loop {
        let sleep_nanos = loop {
            if serv.fetch_and_handle(&mut handler).is_err() {
                break 'outer;
            }

            // Check if we got some messages through our main rgate.
            handler.fetch_and_handle();

            // receive events from clients and push data to send into smoltcp sockets
            let sends_pending = handler.process_incoming();

            let cur_time = smoltcp::time::Instant::from_millis(start.elapsed().as_millis() as i64);

            // now poll smoltcp to send and receive packets
            if let Err(e) = handler.iface.poll(cur_time) {
                log!(LogFlags::NetPoll, "netrs: poll failed: {}", e);
            }

            // check for outgoing events we have to send to clients
            let recvs_pending = handler.process_outgoing();

            if !sends_pending && !recvs_pending && !handler.iface.needs_poll() {
                // ask smoltcp how long we can sleep
                match handler.iface.poll_delay(cur_time) {
                    // we need to call it again immediately => continue the loop
                    Some(d) if d.total_millis() == 0 => continue,
                    // we should not wait longer than `n` => sleep for `n`
                    Some(n) => break TimeDuration::from_millis(n.total_millis()),
                    // smoltcp has nothing to do => sleep until the next TCU message arrives
                    None => break TimeDuration::MAX,
                }
            }
        };

        let now = TimeInstant::now();
        let sleep_nanos = match next_timeout() {
            Some(timeout) if timeout > now && timeout - now < sleep_nanos => timeout - now,
            _ => sleep_nanos,
        };

        log_net(NetLogEvent::StartedWaiting, 0, 0);
        log!(LogFlags::NetPoll, "Sleeping for {:?}", sleep_nanos);
        OwnActivity::sleep_for(sleep_nanos).ok();
        log_net(NetLogEvent::StoppedWaiting, 0, 0);
    }

    Ok(())
}
