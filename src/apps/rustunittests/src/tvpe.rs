/*
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

use m3::com::{SendGate, SGateArgs, RecvGate};
use m3::boxed::Box;
use m3::env;
use m3::test;
use m3::util;
use m3::vpe::{Activity, VPE, VPEArgs};

pub fn run(t: &mut dyn test::Tester) {
    #[cfg(not(target_arch = "arm"))]
    run_test!(t, run_stop);
    run_test!(t, run_arguments);
    run_test!(t, run_send_receive);
    #[cfg(target_os = "none")]
    run_test!(t, exec_fail);
    run_test!(t, exec_hello);
    run_test!(t, exec_rust_hello);
}

// this doesn't work on arm yet, because of problems in rctmux
#[cfg(not(target_arch = "arm"))]
fn run_stop() {
    use m3::com::recv_msg;
    use m3::com::RGateArgs;
    use m3::dtu::DTU;
    use m3::vfs;

    let mut rg = assert_ok!(RecvGate::new_with(RGateArgs::new().order(6).msg_order(6)));
    assert_ok!(rg.activate());

    let mut wait_time = 10000;
    for _ in 1..100 {
        let mut vpe = assert_ok!(VPE::new_with(VPEArgs::new("test")));

        // pass sendgate to child
        let sg = assert_ok!(SendGate::new_with(SGateArgs::new(&rg).credits(64)));
        assert_ok!(vpe.delegate_obj(sg.sel()));

        // pass root fs to child
        let rootmnt = assert_some!(VPE::cur().mounts().get_by_path("/"));
        assert_ok!(vpe.mounts().add("/", rootmnt));
        assert_ok!(vpe.obtain_mounts());

        let act = assert_ok!(vpe.run(Box::new(move || {
            // notify parent that we're running
            assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));
            let mut _n = 0;
            loop {
                _n += 1;
                // just to execute more interesting instructions than arithmetic or jumps
                vfs::VFS::stat("/").ok();
            }
        })));

        // wait for child
        assert_ok!(recv_msg(&rg));

        // wait a bit and stop VPE
        assert_ok!(DTU::sleep(wait_time));
        assert_ok!(act.stop());

        // increase by one cycle to attempt interrupts at many points in the instruction stream
        wait_time += 1;
    }
}

fn run_arguments() {
    let vpe = assert_ok!(VPE::new_with(VPEArgs::new("test")));

    let act = assert_ok!(vpe.run(Box::new(|| {
        assert_eq!(env::args().count(), 1);
        assert!(env::args().nth(0).is_some());
        assert!(env::args().nth(0).unwrap().ends_with("rustunittests"));
        0
    })));

    assert_eq!(act.wait(), Ok(0));
}

fn run_send_receive() {
    let mut vpe = assert_ok!(VPE::new_with(VPEArgs::new("test")));

    let mut rgate = assert_ok!(RecvGate::new(util::next_log2(256), util::next_log2(256)));
    let sgate = assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(256)));

    assert_ok!(vpe.delegate_obj(rgate.sel()));

    let act = assert_ok!(vpe.run(Box::new(move || {
        assert_ok!(rgate.activate());
        let (i1, i2) = assert_ok!(recv_vmsg!(&rgate, u32, u32));
        assert_eq!((i1, i2), (42, 23));
        (i1 + i2) as i32
    })));

    assert_ok!(send_vmsg!(&sgate, RecvGate::def(), 42, 23));

    assert_eq!(act.wait(), Ok(42 + 23));
}

#[cfg(target_os = "none")]
fn exec_fail() {
    use m3::errors::Code;

    // file too small
    {
        let vpe = assert_ok!(VPE::new_with(VPEArgs::new("test")));
        let act = vpe.exec(&["/testfile.txt"]);
        assert!(act.is_err() && act.err().unwrap().code() == Code::EndOfFile);
    }

    // not an ELF file
    {
        let vpe = assert_ok!(VPE::new_with(VPEArgs::new("test")));
        let act = vpe.exec(&["/pat.bin"]);
        assert!(act.is_err() && act.err().unwrap().code() == Code::InvalidElf);
    }
}

fn exec_hello() {
    let vpe = assert_ok!(VPE::new_with(VPEArgs::new("test")));

    let act = assert_ok!(vpe.exec(&["/bin/hello"]));
    assert_eq!(act.wait(), Ok(0));
}

fn exec_rust_hello() {
    let vpe = assert_ok!(VPE::new_with(VPEArgs::new("test")));

    let act = assert_ok!(vpe.exec(&["/bin/rusthello"]));
    assert_eq!(act.wait(), Ok(0));
}
