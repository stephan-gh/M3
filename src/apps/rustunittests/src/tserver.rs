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
use m3::cap::Selector;
use m3::col::String;
use m3::com::{RecvGate, RGateArgs, SendGate, SGateArgs, recv_msg};
use m3::dtu;
use m3::errors::{Code, Error};
use m3::kif;
use m3::server::{Handler, Server, SessId, SessionContainer, server_loop};
use m3::session::{ClientSession, ServerSession};
use m3::test;
use m3::vpe::{Activity, VPE, VPEArgs};

pub fn run(t: &mut dyn test::Tester) {
    run_test!(t, testnoresp);
    run_test!(t, testcliexit);
    run_test!(t, testmsgs);
    run_test!(t, testcaps);
}

struct MySession {
    _sess: ServerSession,
}

struct MyHandler {
    sessions: SessionContainer<MySession>,
}

impl Handler for MyHandler {
    fn open(&mut self, srv_sel: Selector, _arg: &str) -> Result<(Selector, u64), Error> {
        let sess = ServerSession::new(srv_sel, 0)?;

        let sel = sess.sel();
        // keep the session to ensure that it's not destroyed
        self.sessions.add(MySession {
            _sess: sess,
        });
        Ok((sel, 0))
    }

    fn obtain(&mut self, _: SessId, _: &mut kif::service::ExchangeData) -> Result<(), Error> {
        // don't respond, just exit
        m3::exit(1);
    }

    fn close(&mut self, _: SessId) {
    }
}

impl MyHandler {
    pub fn new() -> Self {
        MyHandler {
            sessions: SessionContainer::new(),
        }
    }
}

fn server_main() -> i32 {
    let s = assert_ok!(Server::new("test"));
    let mut hdl = MyHandler::new();

    server_loop(|| {
        s.handle_ctrl_chan(&mut hdl)
    }).ok();
    0
}

pub fn testnoresp() {
    let client = assert_ok!(VPE::new_with(VPEArgs::new("client")));

    let cact = {
        let serv = assert_ok!(VPE::new_with(VPEArgs::new("server")));

        let sact = assert_ok!(serv.run(Box::new(&server_main)));

        let cact = assert_ok!(client.run(Box::new(|| {
            let sess = loop {
                if let Ok(s) = ClientSession::new("test") {
                    break s;
                }
            };
            assert_err!(sess.obtain_obj(), Code::RecvGone);
            0
        })));

        assert_eq!(sact.wait(), Ok(1));
        cact

        // destroy server VPE to let the client request fail
    };

    // now wait for client
    assert_eq!(cact.wait(), Ok(0));
}

pub fn testcliexit() {
    let mut client = assert_ok!(VPE::new_with(VPEArgs::new("client")));
    let serv = assert_ok!(VPE::new_with(VPEArgs::new("server")));

    let sact = assert_ok!(serv.run(Box::new(&server_main)));

    let mut rg = assert_ok!(RecvGate::new_with(RGateArgs::new().order(7).msg_order(6)));
    assert_ok!(rg.activate());

    let sg = assert_ok!(SendGate::new_with(SGateArgs::new(&rg).credits(64 * 2)));
    assert_ok!(client.delegate_obj(sg.sel()));

    let cact = assert_ok!(client.run(Box::new(move || {
        let sess = loop {
            if let Ok(s) = ClientSession::new("test") {
                break s;
            }
        };

        // first send to activate the gate
        assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));

        // perform the obtain syscall
        let req = kif::syscalls::ExchangeSess {
            opcode: kif::syscalls::Operation::OBTAIN.val,
            vpe_sel: VPE::cur().sel() as u64,
            sess_sel: sess.sel() as u64,
            crd: 0,
            args: kif::syscalls::ExchangeArgs::default(),
        };
        let msg_ptr = &req as *const kif::syscalls::ExchangeSess as *const u8;
        let msg_size = m3::util::size_of::<kif::syscalls::ExchangeSess>();
        assert_ok!(dtu::DTU::send(dtu::SYSC_SEP, msg_ptr, msg_size, 0, dtu::SYSC_REP));

        // now we're ready to be killed
        assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));

        // wait here; don't exit (we don't have credits anymore)
        loop {}
    })));

    // wait until the child is ready to be killed
    assert_ok!(recv_msg(&rg));
    assert_ok!(recv_msg(&rg));

    assert_eq!(sact.wait(), Ok(1));
    assert_ok!(cact.stop());
}

pub fn testmsgs() {
    {
        let sess = assert_ok!(ClientSession::new("testmsgs"));
        let sel = assert_ok!(sess.obtain_obj());
        let mut sgate = SendGate::new_bind(sel);

        for _ in 0..5 {
            let mut reply = assert_ok!(send_recv!(&mut sgate, RecvGate::def(), 0, "123456"));
            let resp: String = reply.pop();
            assert_eq!(resp, "654321");
        }
    }

    {
        let sess = assert_ok!(ClientSession::new("testmsgs"));
        let sel = assert_ok!(sess.obtain_obj());
        let mut sgate = SendGate::new_bind(sel);

        let mut reply = assert_ok!(send_recv!(&mut sgate, RecvGate::def(), 0, "123456"));
        let resp: String = reply.pop();
        assert_eq!(resp, "654321");

        assert_err!(send_recv!(&mut sgate, RecvGate::def(), 0, "123456"), Code::InvEP, Code::RecvGone);
    }
}

pub fn testcaps() {
    for _ in 0..10 {
        let sess = loop {
            let sess_res = ClientSession::new("testcaps");
            if let Result::Ok(sess) = sess_res {
                break sess;
            }
        };

        for _ in 0..5 {
            assert_err!(sess.obtain_obj(), Code::NotSup);
        }

        assert_err!(sess.obtain_obj(), Code::InvArgs, Code::RecvGone);
    }

    assert_err!(ClientSession::new("testcaps"), Code::InvArgs);
}
