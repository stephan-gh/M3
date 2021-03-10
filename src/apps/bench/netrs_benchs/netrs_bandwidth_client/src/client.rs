#![no_std]

#[macro_use]
extern crate m3;

use m3::{profile, tcu::TCUIf, cell::StaticCell, col::{BoxList, BoxRef}, com::Semaphore, net::{IpAddr, TcpSocket, UdpSocket}, println, profile::Profiler, session::NetworkManager, test::{self, WvTester}, time::{self, Time}};

// TODO that's hacky, but the only alternative I can see is to pass the WvTester to every single
// test case and every single wv_assert_* call, which is quite inconvenient.
static FAILED: StaticCell<u32> = StaticCell::new(0);

extern "C" fn wvtest_failed() {
    FAILED.set(*FAILED + 1);
}

struct MyTester {}

static PACKETS_TO_SEND: usize = 105;
static PACKETS_TO_RECEIVE: usize = 100;
static BURST_SIZE: usize = 2;

struct NetContext<'a>{
    socket: UdpSocket<'a>,
    req_buffer: [u8; 1024],
    dest_addr: IpAddr,
    dest_port: u16,
    sent_count: usize,
    receive_count: usize,
    received_bytes: usize
}


impl WvTester for MyTester {
    fn run_suite(&mut self, name: &str, f: &dyn Fn(&mut dyn WvTester)) {
        println!("Running benchmark suite {} ...\n", name);
        f(self);
        println!();
    }

    fn run_test(&mut self, name: &str, file: &str, f: &dyn Fn()) {
        println!("Testing \"{}\" in {}:", name, file);
        f();
        println!();
    }
}

fn run(t: &mut dyn test::WvTester){
    wv_run_test!(t, simple_bandwidth);
}

fn simple_bandwidth(){
    let mut prof = Profiler::default().repeats(5);


    //Setup context
    let nm = NetworkManager::new("net0").unwrap();
    let mut socket = UdpSocket::new(&nm).unwrap();

    socket.set_blocking(true);
    Semaphore::attach("net").unwrap().down();

    socket.bind(IpAddr::new(192, 168, 112, 2), 1337).unwrap();

    socket.set_blocking(false);
    let mut context = NetContext{
	socket,
	req_buffer: [0; 1024],
	dest_addr: IpAddr::new(192, 168, 112, 1),
	dest_port: 1337,
	sent_count: 0,
	receive_count: 0,
	received_bytes: 0
    };

    
    
    wv_perf!(
        "running bandwidth test",
        prof.run_with_id(
	    || {
		let mut failures = 0;
		loop{
		    if failures > 9{
			failures = 0;
			TCUIf::sleep();
		    }
		    for _i in 0..BURST_SIZE{
			if context.sent_count > PACKETS_TO_SEND{
			    break;
			}
			context.socket.send(context.dest_addr, context.dest_port, &context.req_buffer).unwrap();
			context.sent_count += 1;
			failures = 0;
		    }

		    let receive_count = BURST_SIZE;
		    for _ in 0..receive_count{
			if let Ok(pkg) = context.socket.recv(){
			    context.received_bytes += pkg.size as usize;
			    context.receive_count += 1;
			}else{
			    failures += 1;
			}
		    }
		    
		    if context.receive_count >= PACKETS_TO_RECEIVE{
			break;
		    }
		    if (context.sent_count >= PACKETS_TO_SEND){
			break;
		    }
		}
	    },
	    0xa1
	)
    );
    
}

#[no_mangle]
pub fn main() -> i32 {
    let mut tester = MyTester{};
    wv_run_suite!(tester, run);

    println!("Finished");
    if *FAILED > 0 {
        println!("\x1B[1;31m{} tests failed\x1B[0;m", *FAILED);
    }
    else {
        println!("\x1B[1;32mAll tests successful!\x1B[0;m");
    }
    
    0
}
