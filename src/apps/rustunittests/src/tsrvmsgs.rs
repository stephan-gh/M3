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
use m3::cell::LazyStaticRefCell;
use m3::col::String;
use m3::com::{GateIStream, RecvGate, SendGate};
use m3::errors::{Code, Error};
use m3::kif;
use m3::math::next_log2;
use m3::pes::{Activity, VPEArgs, PE, VPE};
use m3::server::{server_loop, CapExchange, Handler, Server, SessId, SessionContainer};
use m3::session::ServerSession;
use m3::test;
use m3::{reply_vmsg, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, testmsgs);
}

struct MsgSession {
    _sess: ServerSession,
    sgate: SendGate,
}

struct MsgHandler {
    sessions: SessionContainer<MsgSession>,
    calls: u32,
}

static RGATE: LazyStaticRefCell<RecvGate> = LazyStaticRefCell::default();

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
        let sgate = wv_assert_ok!(SendGate::new(&RGATE.borrow()));
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

        let rgate = RGATE.borrow();
        if let Some(msg) = rgate.fetch() {
            let mut is = GateIStream::new(msg, &rgate);
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
    use m3::send_recv;

    let server_pe = wv_assert_ok!(PE::get("clone|own"));
    let serv = wv_assert_ok!(VPE::new_with(server_pe, VPEArgs::new("server")));
    let sact = wv_assert_ok!(serv.run(server_msgs_main));

    {
        let sess = crate::tserver::connect("test");
        let sel = wv_assert_ok!(sess.obtain_obj());
        let sgate = SendGate::new_bind(sel);

        for _ in 0..5 {
            let mut reply = wv_assert_ok!(send_recv!(&sgate, RecvGate::def(), "123456"));
            let resp: String = wv_assert_ok!(reply.pop());
            wv_assert_eq!(resp, "654321");
        }
    }

    {
        let sess = crate::tserver::connect("test");
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
