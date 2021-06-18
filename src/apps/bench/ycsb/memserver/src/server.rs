/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#![no_std]
#![allow(incomplete_features)]
#![feature(inline_const)]
#![feature(array_methods)]

use m3::{
    com::Semaphore,
    net::{StreamSocketArgs, TcpSocket},
    session::NetworkManager,
    tcu::TCU,
    vec::Vec,
};

#[macro_use]
extern crate m3;

extern crate hashbrown;

#[path = "../../ycsbclient/src/importer.rs"]
mod importer;

mod database;
use database::KeyValueStore;

const BUFFER_SIZE: usize = 8 * 1024;

#[no_mangle]
pub fn main() -> i32 {
    let mut kv = KeyValueStore::new();

    let mut recv_timing = 0;
    let mut op_timing = 0;

    // Setup context
    let nm = NetworkManager::new("net0").unwrap();
    let mut socket = TcpSocket::new(
        StreamSocketArgs::new(&nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .unwrap();

    socket.listen(1337).unwrap();
    Semaphore::attach("net").unwrap().up().unwrap();
    socket.accept().unwrap();

    let mut package_buffer = Vec::with_capacity(BUFFER_SIZE);
    package_buffer.resize(BUFFER_SIZE, 0 as u8);

    let mut opcounter = 0;
    loop {
        // Receiving a package is a two step process. First we receive a u32, which carries the
        // number of bytes the following package is big.
        //
        // We then wait until we received all those bytes. After that the package is parsed and send
        // to the database.
        let recv_start = TCU::nanotime();
        // First receive package size header
        let mut pkg_size_header = [0 as u8; 4];
        let mut pkg_header_ptr = 0;
        while pkg_header_ptr < pkg_size_header.len() {
            match socket.recv(&mut pkg_size_header[pkg_header_ptr..]) {
                Ok(recv_size) => {
                    pkg_header_ptr += recv_size;
                },
                Err(e) => panic!("receive failed: {}", e),
            }
        }

        // Receive the next package from the socket
        let package_size = (u32::from_be_bytes(pkg_size_header)) as usize;
        if package_size > package_buffer.len() {
            println!("Invalid package header, was {}b long", package_size);
            continue;
        }

        let mut pkg_ptr = 0;
        while pkg_ptr < package_size {
            match socket.recv(&mut package_buffer[pkg_ptr..package_size]) {
                Ok(rs) => pkg_ptr += rs,
                Err(e) => panic!("receive failed: {}", e),
            }
        }

        // There is an edge case where the package size is 6, If thats the case, check if we got the
        // end flag from the client. In that case its time to stop the benchmark.
        if package_size == 6 {
            if &package_buffer[0..6] == b"ENDNOW" {
                break;
            }
        }

        recv_timing += TCU::nanotime() - recv_start;

        // Try to read bytes as package and execute them.
        let op_start = TCU::nanotime();
        match importer::Package::from_bytes(&package_buffer[0..package_size]) {
            Ok((_read, pkg)) => {
                // Give a feedback that we are working
                if (opcounter % 100) == 0 {
                    println!("Op={} @ {}", pkg.op, opcounter)
                }

                opcounter += 1;
                if let Err(_) = kv.execute(pkg) {
                    println!("Error while executing");
                }

                if opcounter % 16 == 0 {
                    socket.send(&[0]).expect("send failed");
                }

                op_timing += TCU::nanotime() - op_start;
            },
            Err(should_abort) => {
                if should_abort {
                    println!("Aboard @ {}", opcounter);
                    // Reset package and issue warning. Happens if a package is corrupted.
                    println!("WARNING: Aborting operation, package corrupted");
                }
            },
        }
    }

    // wait a bit to ensure that the client is finished with its prints
    let var = 0;
    for _ in 0..500000 {
        unsafe {
            let _ = core::ptr::read_volatile(&var);
        }
    }

    println!("Server Side:");
    println!("    avg recv time: {}ns", recv_timing / opcounter);
    println!("    avg op time:   {}ns", op_timing / opcounter);
    kv.print_stats(opcounter as usize);
    0
}
