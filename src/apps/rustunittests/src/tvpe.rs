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

use m3::boxed::Box;
use m3::com::{RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::pes::{Activity, PE, VPEArgs, VPE};
use m3::test;
use m3::util;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, run_stop);
    wv_run_test!(t, run_arguments);
    wv_run_test!(t, run_send_receive);
    #[cfg(target_os = "none")]
    wv_run_test!(t, exec_fail);
    wv_run_test!(t, exec_hello);
    wv_run_test!(t, exec_rust_hello);
}

fn run_stop() {
    use m3::com::recv_msg;
    use m3::com::RGateArgs;
    use m3::dtu::DTUIf;
    use m3::vfs;

    let mut rg = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(6).msg_order(6)
    ));
    wv_assert_ok!(rg.activate());

    let pe = wv_assert_ok!(PE::new(&VPE::cur().pe_desc()));

    let mut wait_time = 10000;
    for _ in 1..100 {
        let mut vpe = wv_assert_ok!(VPE::new_with(&pe, VPEArgs::new("test")));

        // pass sendgate to child
        let sg = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rg).credits(64)));
        wv_assert_ok!(vpe.delegate_obj(sg.sel()));

        // pass root fs to child
        let rootmnt = wv_assert_some!(VPE::cur().mounts().get_by_path("/"));
        wv_assert_ok!(vpe.mounts().add("/", rootmnt));
        wv_assert_ok!(vpe.obtain_mounts());

        let act = wv_assert_ok!(vpe.run(Box::new(move || {
            // notify parent that we're running
            wv_assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));
            let mut _n = 0;
            loop {
                _n += 1;
                // just to execute more interesting instructions than arithmetic or jumps
                vfs::VFS::stat("/").ok();
            }
        })));

        // wait for child
        wv_assert_ok!(recv_msg(&rg));

        // wait a bit and stop VPE
        wv_assert_ok!(DTUIf::sleep_for(wait_time));
        wv_assert_ok!(act.stop());

        // increase by one cycle to attempt interrupts at many points in the instruction stream
        wait_time += 1;
    }
}

fn run_arguments() {
    let pe = wv_assert_ok!(PE::new(&VPE::cur().pe_desc()));
    let vpe = wv_assert_ok!(VPE::new_with(&pe, VPEArgs::new("test")));

    let act = wv_assert_ok!(vpe.run(Box::new(|| {
        wv_assert_eq!(env::args().count(), 1);
        assert!(env::args().nth(0).is_some());
        assert!(env::args().nth(0).unwrap().ends_with("rustunittests"));
        0
    })));

    wv_assert_eq!(act.wait(), Ok(0));
}

fn run_send_receive() {
    let pe = wv_assert_ok!(PE::new(&VPE::cur().pe_desc()));
    let mut vpe = wv_assert_ok!(VPE::new_with(&pe, VPEArgs::new("test")));

    let mut rgate = wv_assert_ok!(RecvGate::new(util::next_log2(256), util::next_log2(256)));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(256)));

    wv_assert_ok!(vpe.delegate_obj(rgate.sel()));

    let act = wv_assert_ok!(vpe.run(Box::new(move || {
        wv_assert_ok!(rgate.activate());
        let (i1, i2) = wv_assert_ok!(recv_vmsg!(&rgate, u32, u32));
        wv_assert_eq!((i1, i2), (42, 23));
        (i1 + i2) as i32
    })));

    wv_assert_ok!(send_vmsg!(&sgate, RecvGate::def(), 42, 23));

    wv_assert_eq!(act.wait(), Ok(42 + 23));
}

#[cfg(target_os = "none")]
fn exec_fail() {
    use m3::errors::Code;

    let pe = wv_assert_ok!(PE::new(&VPE::cur().pe_desc()));
    // file too small
    {
        let vpe = wv_assert_ok!(VPE::new_with(&pe, VPEArgs::new("test")));
        let act = vpe.exec(&["/testfile.txt"]);
        assert!(act.is_err() && act.err().unwrap().code() == Code::EndOfFile);
    }

    // not an ELF file
    {
        let vpe = wv_assert_ok!(VPE::new_with(&pe, VPEArgs::new("test")));
        let act = vpe.exec(&["/pat.bin"]);
        assert!(act.is_err() && act.err().unwrap().code() == Code::InvalidElf);
    }
}

fn exec_hello() {
    let pe = wv_assert_ok!(PE::new(&VPE::cur().pe_desc()));
    let vpe = wv_assert_ok!(VPE::new_with(&pe, VPEArgs::new("test")));

    let act = wv_assert_ok!(vpe.exec(&["/bin/hello"]));
    wv_assert_eq!(act.wait(), Ok(0));
}

fn exec_rust_hello() {
    let pe = wv_assert_ok!(PE::new(&VPE::cur().pe_desc()));
    let vpe = wv_assert_ok!(VPE::new_with(&pe, VPEArgs::new("test")));

    let act = wv_assert_ok!(vpe.exec(&["/bin/rusthello"]));
    wv_assert_eq!(act.wait(), Ok(0));
}
