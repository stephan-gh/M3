/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

use m3::cell::StaticCell;
use m3::col::String;
use m3::com::{GateIStream, RecvGate};
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::log;
use m3::server::{RequestHandler, RequestSession, Server, ServerSession};
use m3::test::WvTester;
use m3::tiles::{ActivityArgs, ChildActivity, OwnActivity, RunningActivity, Tile};

use m3::{reply_vmsg, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, testmsgs);
}

static CALLS: StaticCell<u64> = StaticCell::new(0);

struct MsgSession {
    _serv: ServerSession,
}

impl RequestSession for MsgSession {
    fn new(_serv: ServerSession, _arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(Self { _serv })
    }
}

impl MsgSession {
    fn test(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let s: &str = is.pop()?;
        log!(LogFlags::Debug, "test({})", s);

        let mut res = String::new();
        for c in s.chars().rev() {
            res.push(c);
        }
        reply_vmsg!(is, res)?;

        // pretend that we crash after some requests
        CALLS.set(CALLS.get() + 1);
        if CALLS.get() == 6 {
            OwnActivity::exit_with(Code::EndOfFile);
        }
        Ok(())
    }
}

fn server_msgs_main() -> Result<(), Error> {
    let mut hdl = wv_assert_ok!(RequestHandler::new());
    let mut srv = wv_assert_ok!(Server::new("test", &mut hdl));

    hdl.reg_msg_handler(0usize, MsgSession::test);

    hdl.run(&mut srv)
}

fn testmsgs(t: &mut dyn WvTester) {
    use m3::send_recv;

    let server_tile = wv_assert_ok!(Tile::get("compat|own"));
    let serv = wv_assert_ok!(ChildActivity::new_with(
        server_tile,
        ActivityArgs::new("server")
    ));
    let sact = wv_assert_ok!(serv.run(server_msgs_main));

    {
        let sess = crate::tserver::open_sess("test");
        let sgate = wv_assert_ok!(sess.connect());

        for _ in 0..5 {
            let mut reply = wv_assert_ok!(send_recv!(&sgate, RecvGate::def(), 0, "123456"));
            let resp: String = wv_assert_ok!(reply.pop());
            wv_assert_eq!(t, resp, "654321");
        }
    }

    {
        let sess = crate::tserver::open_sess("test");
        let sgate = wv_assert_ok!(sess.connect());

        let mut reply = wv_assert_ok!(send_recv!(&sgate, RecvGate::def(), 0, "123456"));
        let resp: String = wv_assert_ok!(reply.pop());
        wv_assert_eq!(t, resp, "654321");

        wv_assert_err!(
            t,
            send_recv!(&sgate, RecvGate::def(), "123456"),
            Code::NoSEP,
            Code::RecvGone
        );
    }

    wv_assert_eq!(t, sact.wait(), Ok(Code::EndOfFile));
}
