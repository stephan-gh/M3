#![no_std]

#[macro_use]
extern crate m3;

use m3::{
    com::Semaphore,
    errors::Code,
    net::{Endpoint, IpAddr, StreamSocketArgs, TcpSocket},
    pes::VPE,
    println,
    session::NetworkManager,
    tcu::TCU,
    vfs::{BufReader, OpenFlags},
};

mod importer;

#[no_mangle]
pub fn main() -> i32 {
    let prg_start = TCU::nanotime();

    //Mount fs to load binary data
    m3::vfs::VFS::mount("/", "m3fs", "m3fs").expect("Failed to mount root filesystem on server");
    let workload = match m3::vfs::VFS::open("/data/workload.wl", OpenFlags::R) {
        Ok(file) => file,
        Err(e) => {
            println!("Could not open file: {:?}", e);
            return 1;
        },
    };

    //Connect to server
    let startup_start = TCU::nanotime();
    let nm = if let Ok(nm) = NetworkManager::new("net0") {
        nm
    }
    else {
        println!("Could not connect to network manager");
        return 1;
    };
    let mut socket = TcpSocket::new(
        StreamSocketArgs::new(&nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .unwrap();
    socket.set_blocking(true);

    //Wait for server to listen
    Semaphore::attach("net").unwrap().down().unwrap();
    socket
        .connect(Endpoint::new(IpAddr::new(192, 168, 112, 1), 1337))
        .unwrap();

    let startup = TCU::nanotime() - startup_start;

    //Load workload info for the benchmark
    let mut workload_buffer = BufReader::new(workload);
    let workload_header = importer::WorkloadHeader::load_from_file(&mut workload_buffer);

    let mut send_recv_time: u64 = 0;
    let mut num_send_bytes: u64 = 0;

    let com_start = TCU::nanotime();

    for _idx in 0..workload_header.number_of_operations {
        let operation = importer::Package::load_as_bytes(&mut workload_buffer);
        num_send_bytes += operation.len() as u64;
        debug_assert!(importer::Package::from_bytes(&operation).is_ok());

        let this_send_recv = TCU::nanotime();

        match socket.send(&(operation.len() as u32).to_be_bytes()) {
            Ok(_s) => {},
            Err(e) => match e.code() {
                Code::WouldBlock => {
                    VPE::sleep().unwrap();
                },
                _ => {
                    println!("Failed sending package length: {}", e);
                    break;
                },
            },
        }
        match socket.send(&operation) {
            Ok(_s) => {},
            Err(e) => match e.code() {
                Code::WouldBlock => {
                    VPE::sleep().unwrap();
                },
                _ => {
                    println!("ERROR: Send failed: {}", e);
                    break;
                },
            },
        }
        send_recv_time += TCU::nanotime() - this_send_recv;
    }
    let com_time = TCU::nanotime() - com_start;

    let end_msg = b"ENDNOW";
    socket.send(&(end_msg.len() as u32).to_be_bytes()).unwrap();
    socket.send(end_msg).unwrap();

    println!("----YCSB benchmark----");
    println!("Client Side:");
    println!(
        "    Whole benchmark took      {:.4}ms",
        (TCU::nanotime() - prg_start) as f32 / 1_000_000.0
    );
    println!("    Startup took:             {}ns", startup);
    println!(
        "    Avg communication time:   {:.4}ms",
        com_time as f64 / workload_header.number_of_operations as f64 / 1_000_000.0
    );
    println!(
        "    Avg send -> receive time: {:.4}ms",
        send_recv_time as f64 / workload_header.number_of_operations as f64 / 1_000_000.0
    );
    //Taken from the bandwidth benchmark
    let duration = send_recv_time;
    let mbps = (num_send_bytes as f32 / 1_000_000.0) / (duration as f32 / 1_000_000_000.0);
    println!("    Throughput:               {:.4}mb/s", mbps);
    println!(
        "    Send Data                 {:.4}mb",
        num_send_bytes as f32 / (1024 * 1024) as f32
    );
    0
}
