#![no_std]
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

#[path = "../../netrs_ycsb_bench_client/src/importer.rs"]
mod importer;

mod database;
use database::KeyValueStore;

const BUFFER_SIZE: usize = 8 * 1024;

#[no_mangle]
pub fn main() -> i32 {
    let mut kv = KeyValueStore::new();

    let mut recv_timing = 0;
    let mut op_timing = 0;

    //Setup context
    let nm = NetworkManager::new("net1").unwrap();
    let mut socket = TcpSocket::new(
        StreamSocketArgs::new(&nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .unwrap();

    socket.listen(1337).unwrap();
    socket.set_blocking(true);
    Semaphore::attach("net").unwrap().up().unwrap();
    socket.accept().unwrap();
    socket.set_blocking(false);

    let mut package_buffer = Vec::with_capacity(BUFFER_SIZE);
    package_buffer.resize(BUFFER_SIZE, 0 as u8);

    let mut opcounter = 0;
    loop {
        //Receiving a package is a two step process. First we receive a u32, which carries the number of bytes the
        //following package is big.
        //
        //We then wait until we received all those bytes. After that the package is parsed and send to the database.
        let recv_start = TCU::nanotime();
        //First receive package size header
        let mut pkg_size_header = [0 as u8; 4];
        let mut pkg_header_ptr = 0;
        while pkg_header_ptr < pkg_size_header.len() {
            match socket.recv(&mut pkg_size_header[pkg_header_ptr..]) {
                Ok(recv_size) => {
                    pkg_header_ptr += recv_size;
                },
                Err(_) => continue,
            }
        }

        //Receive the next package from the socket
        let package_size = (u32::from_be_bytes(pkg_size_header)) as usize;
        if package_size > package_buffer.len() {
            println!("Invalid package header, was {}", package_size);
            continue;
        }

        let mut pkg_ptr = 0;
        while pkg_ptr < package_size {
            match socket.recv(&mut package_buffer[pkg_ptr..package_size]) {
                Ok(rs) => pkg_ptr += rs,
                Err(_e) => {
                    continue;
                },
            }
        }

        //There is an edge case where the package size is 6, If thats the case, check if we got the end flag
        //from the client. In that case its time to stop the benchmark.
        if package_size == 6 {
            if &package_buffer[0..6] == b"ENDNOW" {
                break;
            }
        }

        recv_timing += TCU::nanotime() - recv_start;

        //Try to read bytes as package and execute them.
        let op_start = TCU::nanotime();
        match importer::Package::from_bytes(&package_buffer[0..package_size]) {
            Ok((_read, pkg)) => {
                //Give a feedback that we are working
                if (opcounter % 100) == 0 {
                    println!("Op={} @ {}", pkg.op, opcounter)
                }

                opcounter += 1;
                match kv.execute(pkg) {
                    Ok(None) => {},
                    Ok(Some(_pkg)) => {}, //TODO: Could send answer back
                    Err(_) => println!("Err while executing"), //TODO: Was some error while executing, should analyse?
                }

                op_timing += TCU::nanotime() - op_start;
            },
            Err(should_abort) => {
                if should_abort {
                    println!("Aboard @ {}", opcounter);
                    //Reset package and issue warning. Happens if a package is corrupted.
                    println!("WARNING: Aborting operation, package corrupted");
                }
            },
        }
    }
    println!("Server Side:");
    println!(
        "    avg recv timing: {}ms",
        (recv_timing as f32 / opcounter as f32) / 1_000_000.0
    );
    println!(
        "    avg op timing:   {}ms",
        (op_timing as f32 / opcounter as f32) / 1_000_000.0
    );
    kv.print_stats(opcounter);
    0
}
