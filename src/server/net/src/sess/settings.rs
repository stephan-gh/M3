/*
 * Copyright (C) 2021-2023 Nils Asmussen, Barkhausen Institut
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

use base::io::LogFlags;

use m3::col::Vec;
use m3::errors::{Code, Error};
use m3::log;
use m3::net::Port;
use m3::util::parse;

use crate::ports;

pub struct Settings {
    pub bufs: usize,
    pub socks: usize,
    pub raw: bool,
    pub tcp_ports: Vec<(Port, Port)>,
    pub udp_ports: Vec<(Port, Port)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bufs: 64 * 1024,
            socks: 4,
            raw: false,
            tcp_ports: Vec::new(),
            udp_ports: Vec::new(),
        }
    }
}

fn parse_ports(port_descs: &str, ports: &mut Vec<(Port, Port)>) -> Result<(), Error> {
    // comma separated list of "x-y" or "x"
    for port_desc in port_descs.split(',') {
        let range = if let Some(pos) = port_desc.find('-') {
            let from = parse::int(&port_desc[0..pos])? as Port;
            let to = parse::int(&port_desc[(pos + 1)..])? as Port;
            (from, to)
        }
        else {
            let port = parse::int(port_desc)? as Port;
            (port, port)
        };

        if ports::is_ephemeral(range.0) || ports::is_ephemeral(range.1) {
            log!(LogFlags::Error, "Cannot bind/listen on ephemeral ports");
            return Err(Error::new(Code::InvArgs));
        }

        ports.push(range);
    }
    Ok(())
}

pub fn parse_arguments(args_str: &str) -> Result<Settings, Error> {
    let mut args = Settings::default();
    for arg in args_str.split_whitespace() {
        if let Some(bufs) = arg.strip_prefix("bufs=") {
            args.bufs = parse::size(bufs)?;
        }
        else if let Some(socks) = arg.strip_prefix("socks=") {
            args.socks = parse::int(socks)? as usize;
        }
        else if arg == "raw=yes" {
            args.raw = true;
        }
        else if let Some(portdesc) = arg.strip_prefix("tcp=") {
            parse_ports(portdesc, &mut args.tcp_ports)?;
        }
        else if let Some(portdesc) = arg.strip_prefix("udp=") {
            parse_ports(portdesc, &mut args.udp_ports)?;
        }
        else {
            return Err(Error::new(Code::InvArgs));
        }
    }
    Ok(args)
}
