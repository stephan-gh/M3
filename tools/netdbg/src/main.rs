/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

mod error;
mod translator;

use std::env;
use std::process::exit;

use crate::error::Error;
use crate::translator::{App, Net, NIC};

fn usage(prog: &str) -> ! {
    eprintln!(
        "Usage: {} [--nic <name>] [--net <tile> <nic> <name>] [--app <tile> <net> <name>]",
        prog
    );
    eprintln!();
    eprintln!(concat!(
        "The tool expects the gem5.log in stdin. To see sent/received network packets at the",
        " NIC level, please enable the Ethernet flag. To see decoded packet dumps, please enable",
        " the EthernetData flag. Otherwise, the tool relies on the DEBUG prints performed by the",
        " log_net() calls in M³."
    ));
    exit(1)
}

fn main() -> Result<(), Error> {
    let mut nics = Vec::default();
    let mut nets = Vec::default();
    let mut apps = Vec::default();

    let args: Vec<String> = env::args().collect();

    if args.len() == 1 || args[1] == "-h" || args[1] == "--help" {
        usage(&args[0]);
    }

    for i in 0..args.len() {
        let arg = &args[i];
        if arg == "--nic" {
            if i + 1 >= args.len() {
                usage(&args[0]);
            }

            let idx = nics.len();
            let name = &args[i + 1];
            nics.push(NIC::new(idx, name.clone()));
        }
        else if arg == "--net" {
            if i + 3 >= args.len() {
                usage(&args[0]);
            }

            let tile = args[i + 1].parse::<u64>()?;
            let nic_name = &args[i + 2];
            let name = &args[i + 3];
            let nic = nics
                .iter()
                .find(|n| n.name() == nic_name)
                .ok_or_else(|| Error::NotFoundNIC(nic_name.to_string()))?;
            nets.push(Net::new(tile, name.clone(), nic.name()));
        }
        else if arg == "--app" {
            if i + 3 >= args.len() {
                usage(&args[0]);
            }

            let tile = args[i + 1].parse::<u64>()?;
            let net_name = &args[i + 2];
            let name = &args[i + 3];
            let net = nets
                .iter()
                .find(|n| n.name() == net_name)
                .ok_or_else(|| Error::NotFoundNet(net_name.to_string()))?;
            apps.push(App::new(tile, name.clone(), net.full_name()));
        }
    }

    crate::translator::translate(&mut nics, &mut nets, &mut apps)
}
