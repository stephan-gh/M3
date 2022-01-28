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

use m3::cap::Selector;
use m3::com::{recv_msg, RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::math;
use m3::pes::{Activity, VPEArgs, PE, VPE};
use m3::test;
use m3::time::TimeDuration;

use m3::{send_vmsg, wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, run_stop);
    wv_run_test!(t, run_arguments);
    wv_run_test!(t, run_send_receive);
    #[cfg(not(target_vendor = "host"))]
    wv_run_test!(t, exec_fail);
    wv_run_test!(t, exec_hello);
    wv_run_test!(t, exec_rust_hello);
}

fn run_stop() {
    use m3::com::RGateArgs;
    use m3::vfs;
    use m3::wv_assert_some;

    let mut rg = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(6).msg_order(6)
    ));
    wv_assert_ok!(rg.activate());

    let pe = wv_assert_ok!(PE::get("clone|own"));

    let mut wait_time = TimeDuration::from_nanos(10000);
    for _ in 1..100 {
        let mut vpe = wv_assert_ok!(VPE::new_with(pe.clone(), VPEArgs::new("test")));

        // pass sendgate to child
        let sg = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rg).credits(1)));
        wv_assert_ok!(vpe.delegate_obj(sg.sel()));

        // pass root fs to child
        let rootmnt = wv_assert_some!(VPE::cur().mounts().get_by_path("/"));
        wv_assert_ok!(vpe.mounts().add("/", rootmnt));
        wv_assert_ok!(vpe.obtain_mounts());

        let mut dst = vpe.data_sink();
        dst.push_word(sg.sel());

        let act = wv_assert_ok!(vpe.run(|| {
            let mut src = VPE::cur().data_source();
            let sg_sel: Selector = src.pop().unwrap();

            // notify parent that we're running
            let sg = SendGate::new_bind(sg_sel);
            wv_assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));
            let mut _n = 0;
            loop {
                _n += 1;
                // just to execute more interesting instructions than arithmetic or jumps
                vfs::VFS::stat("/").ok();
            }
        }));

        // wait for child
        wv_assert_ok!(recv_msg(&rg));

        // wait a bit and stop VPE
        wv_assert_ok!(VPE::sleep_for(wait_time));
        wv_assert_ok!(act.stop());

        // increase by one ns to attempt interrupts at many points in the instruction stream
        wait_time += TimeDuration::from_nanos(1);
    }
}

fn run_arguments() {
    let pe = wv_assert_ok!(PE::get("clone|own"));
    let vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("test")));

    let act = wv_assert_ok!(vpe.run(|| {
        wv_assert_eq!(env::args().count(), 1);
        assert!(env::args().next().is_some());
        assert!(env::args().next().unwrap().ends_with("rustunittests"));
        0
    }));

    wv_assert_eq!(act.wait(), Ok(0));
}

fn run_send_receive() {
    let pe = wv_assert_ok!(PE::get("clone|own"));
    let mut vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("test")));

    let rgate = wv_assert_ok!(RecvGate::new(math::next_log2(256), math::next_log2(256)));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(vpe.delegate_obj(rgate.sel()));

    let mut dst = vpe.data_sink();
    dst.push_word(rgate.sel());

    let act = wv_assert_ok!(vpe.run(|| {
        let mut src = VPE::cur().data_source();
        let rg_sel: Selector = src.pop().unwrap();

        let mut rgate = RecvGate::new_bind(rg_sel, math::next_log2(256), math::next_log2(256));
        wv_assert_ok!(rgate.activate());
        let mut res = wv_assert_ok!(recv_msg(&rgate));
        let i1 = wv_assert_ok!(res.pop::<u32>());
        let i2 = wv_assert_ok!(res.pop::<u32>());
        wv_assert_eq!((i1, i2), (42, 23));
        (i1 + i2) as i32
    }));

    wv_assert_ok!(send_vmsg!(&sgate, RecvGate::def(), 42, 23));

    wv_assert_eq!(act.wait(), Ok(42 + 23));
}

#[cfg(not(target_vendor = "host"))]
fn exec_fail() {
    use m3::errors::Code;

    let pe = wv_assert_ok!(PE::get("clone|own"));
    // file too small
    {
        let vpe = wv_assert_ok!(VPE::new_with(pe.clone(), VPEArgs::new("test")));
        let act = vpe.exec(&["/testfile.txt"]);
        assert!(act.is_err() && act.err().unwrap().code() == Code::EndOfFile);
    }

    // not an ELF file
    {
        let vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("test")));
        let act = vpe.exec(&["/pat.bin"]);
        assert!(act.is_err() && act.err().unwrap().code() == Code::InvalidElf);
    }
}

fn exec_hello() {
    let pe = wv_assert_ok!(PE::get("clone|own"));
    let vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("test")));

    let act = wv_assert_ok!(vpe.exec(&["/bin/hello"]));
    wv_assert_eq!(act.wait(), Ok(0));
}

fn exec_rust_hello() {
    let pe = wv_assert_ok!(PE::get("clone|own"));
    let vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("test")));

    let act = wv_assert_ok!(vpe.exec(&["/bin/rusthello"]));
    wv_assert_eq!(act.wait(), Ok(0));
}
