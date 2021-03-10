/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

use smoltcp::wire::IpEndpoint;

/// Formats a IpEndpoint into an m3 (IpAddr, u16) tuple.
/// Assumes that the IpEndpoint a is Ipv4 address, otherwise this will panic.
pub fn to_m3_addr(addr: IpEndpoint) -> (m3::net::IpAddr, u16) {
    assert!(addr.addr.as_bytes().len() == 4, "Address was not ipv4!");
    let bytes = addr.addr.as_bytes();
    (
        m3::net::IpAddr::new(bytes[0], bytes[1], bytes[2], bytes[3]),
        addr.port,
    )
}
