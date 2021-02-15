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
use m3::cell::{LazyStaticCell, StaticCell};
use m3::col::String;
use m3::com::{recv_msg, GateIStream, RGateArgs, RecvGate, SGateArgs, SendGate};
use m3::envdata;
use m3::errors::{Code, Error};
use m3::kif;
use m3::math::next_log2;
use m3::pes::{Activity, VPEArgs, PE, VPE};
use m3::println;
use m3::server::{server_loop, CapExchange, Handler, Server, SessId, SessionContainer};
use m3::session::{ClientSession, ServerSession};
use m3::syscalls;
use m3::test;
use m3::{
    reply_vmsg, send_recv, send_vmsg, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test,
};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, testnoresp);
    wv_run_test!(t, testcliexit);
    wv_run_test!(t, testmsgs);
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

    fn obtain(&mut self, _: usize, _: SessId, _: &mut CapExchange) -> Result<(), Error> {
        // don't respond, just exit
        m3::exit(1);
    }
}

fn server_crash_main() -> i32 {
    let mut hdl = CrashHandler {
        sessions: SessionContainer::new(1),
    };
    let s = wv_assert_ok!(Server::new("test", &mut hdl));

    server_loop(|| s.handle_ctrl_chan(&mut hdl)).ok();
    0
}

fn connect(name: &str) -> ClientSession {
    // try to open a session until we succeed. this is required because we start the servers ourself
    // and don't know when they register their service.
    loop {
        let sess_res = ClientSession::new(name);
        if let Result::Ok(sess) = sess_res {
            break sess;
        }
    }
}

pub fn testnoresp() {
    let client_pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let client = wv_assert_ok!(VPE::new_with(client_pe, VPEArgs::new("client")));

    let server_pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let cact = {
        let serv = wv_assert_ok!(VPE::new_with(server_pe, VPEArgs::new("server")));

        let sact = wv_assert_ok!(serv.run(Box::new(&server_crash_main)));

        let cact = wv_assert_ok!(client.run(Box::new(|| {
            let sess = connect("test");
            wv_assert_err!(sess.obtain_obj(), Code::RecvGone);
            0
        })));

        wv_assert_eq!(sact.wait(), Ok(1));
        cact

        // destroy server VPE to let the client request fail
    };

    // now wait for client
    wv_assert_eq!(cact.wait(), Ok(0));
}

pub fn testcliexit() {
    if envdata::get().platform() == envdata::Platform::HW {
        println!("Unsupported because VPEs cannot be stopped remotely");
        return;
    }

    let client_pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let mut client = wv_assert_ok!(VPE::new_with(client_pe, VPEArgs::new("client")));

    let server_pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let serv = wv_assert_ok!(VPE::new_with(server_pe, VPEArgs::new("server")));

    let sact = wv_assert_ok!(serv.run(Box::new(&server_crash_main)));

    let mut rg = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(7).msg_order(6)
    ));
    wv_assert_ok!(rg.activate());

    let sg = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rg).credits(2)));
    wv_assert_ok!(client.delegate_obj(sg.sel()));

    let cact = wv_assert_ok!(client.run(Box::new(move || {
        let sess = loop {
            if let Ok(s) = ClientSession::new("test") {
                break s;
            }
        };

        // first send to activate the gate
        wv_assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));

        // perform the obtain syscall
        let req = kif::syscalls::ExchangeSess {
            opcode: kif::syscalls::Operation::OBTAIN.val,
            vpe_sel: VPE::cur().sel(),
            sess_sel: sess.sel(),
            caps: [0; 2],
            args: kif::syscalls::ExchangeArgs::default(),
        };
        let msg_ptr = &req as *const kif::syscalls::ExchangeSess as *const u8;
        let msg_size = m3::util::size_of::<kif::syscalls::ExchangeSess>();
        wv_assert_ok!(syscalls::send_gate().send_bytes(msg_ptr, msg_size, RecvGate::syscall(), 0,));

        // now we're ready to be killed
        wv_assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));

        // wait here; don't exit (we don't have credits anymore)
        #[allow(clippy::empty_loop)]
        loop {}
    })));

    // wait until the child is ready to be killed
    wv_assert_ok!(recv_msg(&rg));
    wv_assert_ok!(recv_msg(&rg));

    wv_assert_eq!(sact.wait(), Ok(1));
    wv_assert_ok!(cact.stop());
}

struct MsgSession {
    _sess: ServerSession,
    sgate: SendGate,
}

struct MsgHandler {
    sessions: SessionContainer<MsgSession>,
    calls: u32,
}

static RGATE: LazyStaticCell<RecvGate> = LazyStaticCell::default();

impl Handler<MsgSession> for MsgHandler {
    fn sessions(&mut self) -> &mut SessionContainer<MsgSession> {
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
        let sgate = wv_assert_ok!(SendGate::new(&RGATE));
        self.sessions
            .add(crt, 0, MsgSession { _sess: sess, sgate })
            .map(|_| (sel, 0))
    }

    fn obtain(&mut self, _crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        let sess = self.sessions.get(sid).unwrap();
        xchg.out_caps(kif::CapRngDesc::new(
            kif::CapType::OBJECT,
            sess.sgate.sel(),
            1,
        ));
        Ok(())
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        self.sessions.remove(crt, sid);
    }
}

impl MsgHandler {
    fn handle_msg(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        let s: &str = is.pop()?;
        let mut res = String::new();
        for c in s.chars().rev() {
            res.push(c);
        }
        reply_vmsg!(is, res)?;

        // pretend that we crash after some requests
        self.calls += 1;
        if self.calls == 6 {
            m3::exit(1);
        }
        Ok(())
    }
}

fn server_msgs_main() -> i32 {
    let mut hdl = MsgHandler {
        sessions: SessionContainer::new(1),
        calls: 0,
    };
    let s = wv_assert_ok!(Server::new("test", &mut hdl));

    let mut rgate = wv_assert_ok!(RecvGate::new(next_log2(256), next_log2(256)));
    wv_assert_ok!(rgate.activate());
    RGATE.set(rgate);

    server_loop(|| {
        s.handle_ctrl_chan(&mut hdl)?;

        if let Some(msg) = RGATE.fetch() {
            let mut is = GateIStream::new(msg, &RGATE);
            if let Err(e) = hdl.handle_msg(&mut is) {
                is.reply_error(e.code()).ok();
            }
        }
        Ok(())
    })
    .ok();

    0
}

pub fn testmsgs() {
    let server_pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let serv = wv_assert_ok!(VPE::new_with(server_pe, VPEArgs::new("server")));
    let sact = wv_assert_ok!(serv.run(Box::new(&server_msgs_main)));

    {
        let sess = connect("test");
        let sel = wv_assert_ok!(sess.obtain_obj());
        let sgate = SendGate::new_bind(sel);

        for _ in 0..5 {
            let mut reply = wv_assert_ok!(send_recv!(&sgate, RecvGate::def(), "123456"));
            let resp: String = wv_assert_ok!(reply.pop());
            wv_assert_eq!(resp, "654321");
        }
    }

    {
        let sess = connect("test");
        let sel = wv_assert_ok!(sess.obtain_obj());
        let sgate = SendGate::new_bind(sel);

        let mut reply = wv_assert_ok!(send_recv!(&sgate, RecvGate::def(), "123456"));
        let resp: String = wv_assert_ok!(reply.pop());
        wv_assert_eq!(resp, "654321");

        wv_assert_err!(
            send_recv!(&sgate, RecvGate::def(), "123456"),
            Code::NoSEP,
            Code::RecvGone
        );
    }

    wv_assert_eq!(sact.wait(), Ok(1));
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

    fn obtain(&mut self, _: usize, _: SessId, _: &mut CapExchange) -> Result<(), Error> {
        self.calls += 1;
        // stop the service after 5 calls
        if self.calls == 5 {
            *STOP.get_mut() = true;
        }
        Err(Error::new(Code::NotSup))
    }

    fn delegate(&mut self, _: usize, _: SessId, _: &mut CapExchange) -> Result<(), Error> {
        self.calls += 1;
        if self.calls == 5 {
            *STOP.get_mut() = true;
        }
        Err(Error::new(Code::NotSup))
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        self.sessions.remove(crt, sid);
    }
}

fn server_notsup_main() -> i32 {
    for _ in 0..5 {
        *STOP.get_mut() = false;

        let mut hdl = NotSupHandler {
            sessions: SessionContainer::new(1),
            calls: 0,
        };
        let s = wv_assert_ok!(Server::new("test", &mut hdl));

        let res = server_loop(|| {
            if *STOP {
                return Err(Error::new(Code::VPEGone));
            }
            s.handle_ctrl_chan(&mut hdl)
        });
        match res {
            // if there is any other error than our own stop signal, break
            Err(e) if e.code() != Code::VPEGone => break,
            _ => {},
        }
    }

    0
}

pub fn testcaps() {
    let server_pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let serv = wv_assert_ok!(VPE::new_with(server_pe, VPEArgs::new("server")));
    let sact = wv_assert_ok!(serv.run(Box::new(&server_notsup_main)));

    for i in 0..5 {
        let sess = connect("test");

        // test both obtain and delegate
        if i % 2 == 0 {
            for _ in 0..5 {
                wv_assert_err!(sess.obtain_obj(), Code::NotSup);
            }
            wv_assert_err!(sess.obtain_obj(), Code::InvArgs, Code::RecvGone);
        }
        else {
            for _ in 0..5 {
                wv_assert_err!(sess.delegate_obj(sess.sel()), Code::NotSup);
            }
            wv_assert_err!(sess.delegate_obj(sess.sel()), Code::InvArgs, Code::RecvGone);
        }
    }

    wv_assert_err!(ClientSession::new("test"), Code::InvArgs);
    wv_assert_eq!(sact.wait(), Ok(0));
}
