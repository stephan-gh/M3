/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use m3::build_vmsg;
use m3::cap::Selector;
use m3::cell::StaticCell;
use m3::com::{recv_msg, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::kif;
use m3::mem::MsgBuf;
use m3::server::{server_loop, CapExchange, Handler, Server, SessId, SessionContainer};
use m3::session::{ClientSession, ServerSession};
use m3::syscalls;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, OwnActivity, RunningActivity, Tile};
use m3::{send_vmsg, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, testnoresp);
    wv_run_test!(t, testcliexit);
    wv_run_test!(t, testcaps);
}

struct EmptySession {
    _sess: ServerSession,
}

struct CrashHandler {
    sessions: SessionContainer<EmptySession>,
}

impl Handler<EmptySession> for CrashHandler {
    fn sessions(&mut self) -> &mut SessionContainer<EmptySession> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        _arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        let sess = ServerSession::new(srv_sel, crt, 0, false)?;

        let sel = sess.sel();
        // keep the session to ensure that it's not destroyed
        self.sessions
            .add(crt, 0, EmptySession { _sess: sess })
            .map(|_| (sel, 0))
    }

    fn obtain(&mut self, _: usize, _: SessId, _: &mut CapExchange<'_>) -> Result<(), Error> {
        // don't respond, just exit
        OwnActivity::exit_with(Code::EndOfFile);
    }
}

fn server_crash_main() -> Result<(), Error> {
    let mut hdl = CrashHandler {
        sessions: SessionContainer::new(1),
    };
    let s = wv_assert_ok!(Server::new("test", &mut hdl));

    server_loop(|| s.handle_ctrl_chan(&mut hdl)).ok();

    Ok(())
}

pub fn connect(name: &str) -> ClientSession {
    // try to open a session until we succeed. this is required because we start the servers ourself
    // and don't know when they register their service.
    loop {
        let sess_res = ClientSession::new(name);
        if let Result::Ok(sess) = sess_res {
            break sess;
        }
    }
}

fn testnoresp(t: &mut dyn WvTester) {
    let client_tile = wv_assert_ok!(Tile::get("compat|own"));
    let client = wv_assert_ok!(ChildActivity::new_with(
        client_tile,
        ActivityArgs::new("client")
    ));

    let server_tile = wv_assert_ok!(Tile::get("compat|own"));
    let cact = {
        let serv = wv_assert_ok!(ChildActivity::new_with(
            server_tile,
            ActivityArgs::new("server")
        ));

        let sact = wv_assert_ok!(serv.run(server_crash_main));

        let cact = wv_assert_ok!(client.run(|| {
            let mut t = DefaultWvTester::default();
            let sess = connect("test");
            wv_assert_err!(t, sess.obtain_obj(), Code::RecvGone);
            Ok(())
        }));

        wv_assert_eq!(t, sact.wait(), Ok(Code::EndOfFile));
        cact

        // destroy server activity to let the client request fail
    };

    // now wait for client
    wv_assert_eq!(t, cact.wait(), Ok(Code::Success));
}

fn testcliexit(t: &mut dyn WvTester) {
    let client_tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut client = wv_assert_ok!(ChildActivity::new_with(
        client_tile,
        ActivityArgs::new("client")
    ));

    let server_tile = wv_assert_ok!(Tile::get("compat|own"));
    let serv = wv_assert_ok!(ChildActivity::new_with(
        server_tile,
        ActivityArgs::new("server")
    ));

    let sact = wv_assert_ok!(serv.run(server_crash_main));

    let rg = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(7).msg_order(6)
    ));

    let sg = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rg).credits(2)));
    wv_assert_ok!(client.delegate_obj(sg.sel()));

    let mut dst = client.data_sink();
    dst.push(sg.sel());

    let cact = wv_assert_ok!(client.run(|| {
        let mut src = Activity::own().data_source();
        let sg_sel: Selector = src.pop().unwrap();

        let sess = loop {
            if let Ok(s) = ClientSession::new("test") {
                break s;
            }
        };

        // first send to activate the gate
        let sg = SendGate::new_bind(sg_sel);
        wv_assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));

        // ensure that we drop MsgBuf before using send_vmsg below
        {
            // perform the obtain syscall
            let mut req_buf = MsgBuf::borrow_def();
            build_vmsg!(
                req_buf,
                kif::syscalls::Operation::EXCHANGE_SESS,
                kif::syscalls::ExchangeSess {
                    act: Activity::own().sel(),
                    sess: sess.sel(),
                    crd: kif::CapRngDesc::new(kif::CapType::OBJECT, 0, 2),
                    args: kif::syscalls::ExchangeArgs::default(),
                    obtain: true,
                }
            );
            wv_assert_ok!(syscalls::send_gate().send(&req_buf, RecvGate::syscall()));
        }

        // now we're ready to be killed
        wv_assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));

        // wait here; don't exit (we don't have credits anymore)
        #[allow(clippy::empty_loop)]
        loop {}
    }));

    // wait until the child is ready to be killed
    wv_assert_ok!(recv_msg(&rg));
    wv_assert_ok!(recv_msg(&rg));

    wv_assert_eq!(t, sact.wait(), Ok(Code::EndOfFile));
    wv_assert_ok!(cact.stop());
}

static STOP: StaticCell<bool> = StaticCell::new(false);

struct NotSupHandler {
    sessions: SessionContainer<EmptySession>,
    calls: u32,
}

impl Handler<EmptySession> for NotSupHandler {
    fn sessions(&mut self) -> &mut SessionContainer<EmptySession> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        _arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        let sess = ServerSession::new(srv_sel, crt, 0, false)?;

        let sel = sess.sel();
        // keep the session to ensure that it's not destroyed
        self.sessions
            .add(crt, 0, EmptySession { _sess: sess })
            .map(|_| (sel, 0))
    }

    fn obtain(&mut self, _: usize, _: SessId, _: &mut CapExchange<'_>) -> Result<(), Error> {
        self.calls += 1;
        // stop the service after 5 calls
        if self.calls == 5 {
            STOP.set(true);
        }
        Err(Error::new(Code::NotSup))
    }

    fn delegate(&mut self, _: usize, _: SessId, _: &mut CapExchange<'_>) -> Result<(), Error> {
        self.calls += 1;
        if self.calls == 5 {
            STOP.set(true);
        }
        Err(Error::new(Code::NotSup))
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        self.sessions.remove(crt, sid);
    }
}

fn server_notsup_main() -> Result<(), Error> {
    for _ in 0..5 {
        STOP.set(false);

        let mut hdl = NotSupHandler {
            sessions: SessionContainer::new(1),
            calls: 0,
        };
        let s = wv_assert_ok!(Server::new("test", &mut hdl));

        let res = server_loop(|| {
            if STOP.get() {
                return Err(Error::new(Code::ActivityGone));
            }
            s.handle_ctrl_chan(&mut hdl)
        });
        match res {
            // if there is any other error than our own stop signal, break
            Err(e) if e.code() != Code::ActivityGone => break,
            _ => {},
        }
    }

    Ok(())
}

fn testcaps(t: &mut dyn WvTester) {
    let server_tile = wv_assert_ok!(Tile::get("compat|own"));
    let serv = wv_assert_ok!(ChildActivity::new_with(
        server_tile,
        ActivityArgs::new("server")
    ));
    let sact = wv_assert_ok!(serv.run(server_notsup_main));

    for i in 0..5 {
        let sess = connect("test");

        // test both obtain and delegate
        if i % 2 == 0 {
            for _ in 0..5 {
                wv_assert_err!(t, sess.obtain_obj(), Code::NotSup);
            }
            wv_assert_err!(t, sess.obtain_obj(), Code::InvArgs, Code::RecvGone);
        }
        else {
            for _ in 0..5 {
                wv_assert_err!(t, sess.delegate_obj(sess.sel()), Code::NotSup);
            }
            wv_assert_err!(
                t,
                sess.delegate_obj(sess.sel()),
                Code::InvArgs,
                Code::RecvGone
            );
        }
    }

    wv_assert_err!(t, ClientSession::new("test"), Code::InvArgs);
    wv_assert_eq!(t, sact.wait(), Ok(Code::Success));
}
