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

use core::fmt;

use crate::cap::{CapFlags, Capability, Selector};
use crate::com::{GateIStream, RecvGate};
use crate::errors::{Code, Error};
use crate::kif::{service, CapRngDesc};
use crate::llog;
use crate::math;
use crate::mem::MsgBuf;
use crate::serialize::{Sink, Source};
use crate::server::{SessId, SessionContainer};
use crate::syscalls;
use crate::tiles::Activity;

/// Represents a server that provides a service for clients.
pub struct Server {
    cap: Capability,
    rgate: RecvGate,
    public: bool,
}

/// The struct to exchange capabilities with a client (obtain/delegate)
pub struct CapExchange<'d> {
    src: Source<'d>,
    sink: Sink<'d>,
    input: &'d service::ExchangeData,
    out_crd: CapRngDesc,
}

impl<'d> CapExchange<'d> {
    /// Creates a new `CapExchange` object, taking input arguments from `input` and putting output
    /// arguments into `output`.
    pub fn new(input: &'d service::ExchangeData, output: &'d mut service::ExchangeData) -> Self {
        let len = (input.args.bytes as usize + 7) / 8;
        Self {
            src: Source::new(&input.args.data[..len]),
            sink: Sink::new(&mut output.args.data),
            input,
            out_crd: CapRngDesc::default(),
        }
    }

    /// Returns the input arguments
    pub fn in_args(&mut self) -> &mut Source<'d> {
        &mut self.src
    }

    /// Returns the output arguments
    pub fn out_args(&mut self) -> &mut Sink<'d> {
        &mut self.sink
    }

    /// Returns the number of input capabilities
    pub fn in_caps(&self) -> u64 {
        CapRngDesc::new_from(self.input.caps).count()
    }

    /// Sets the output capabilities to given [`CapRngDesc`]
    pub fn out_caps(&mut self, crd: CapRngDesc) {
        self.out_crd = crd;
    }
}

impl<'d> fmt::Debug for CapExchange<'d> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            fmt,
            "CapExchange[in_caps={}, out_crd={}]",
            self.in_caps(),
            self.out_crd,
        )
    }
}

/// The handler for a server that implements the service calls (session creations, cap exchange,
/// ...).
pub trait Handler<S> {
    /// Returns the session container
    fn sessions(&mut self) -> &mut SessionContainer<S>;

    /// Creates a new session with `arg` as an argument for the service with selector `srv_sel`.
    /// Returns the session selector and the session identifier.
    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error>;

    /// Let's the client obtain a capability from the server
    fn obtain(
        &mut self,
        _crt: usize,
        _sid: SessId,
        _xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    /// Let's the client delegate a capability to the server
    fn delegate(
        &mut self,
        _crt: usize,
        _sid: SessId,
        _xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }

    /// Closes the given session
    fn close(&mut self, _crt: usize, _sid: SessId) {
    }

    /// Performs cleanup actions before shutdown
    fn shutdown(&mut self) {
    }
}

const MSG_SIZE: usize = 256;
const BUF_SIZE: usize = MSG_SIZE * (1 + super::sesscon::MAX_CREATORS);

impl Server {
    /// Creates a new server with given service name.
    pub fn new<S>(name: &str, hdl: &mut dyn Handler<S>) -> Result<Self, Error> {
        Self::create(name, hdl, true)
    }

    /// Creates a new private server that is not visible to anyone
    pub fn new_private<S>(name: &str, hdl: &mut dyn Handler<S>) -> Result<Self, Error> {
        Self::create(name, hdl, false)
    }

    fn create<S>(name: &str, hdl: &mut dyn Handler<S>, public: bool) -> Result<Self, Error> {
        let sel = Activity::own().alloc_sel();
        let mut rgate = RecvGate::new(math::next_log2(BUF_SIZE), math::next_log2(MSG_SIZE))?;
        rgate.activate()?;

        syscalls::create_srv(sel, rgate.sel(), name, 0)?;

        let max = hdl.sessions().capacity();
        let (_, sgate) = hdl.sessions().add_creator(&rgate, max)?;

        if public {
            Activity::own()
                .resmng()
                .unwrap()
                .reg_service(sel, sgate, name, max)?;
        }

        Ok(Server {
            cap: Capability::new(sel, CapFlags::empty()),
            rgate,
            public,
        })
    }

    /// Returns the capability selector of the service
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the receive gate that is used for the service protocol
    pub fn rgate(&self) -> &RecvGate {
        &self.rgate
    }

    /// Fetches a message from the control channel and handles it if so.
    pub fn handle_ctrl_chan<S>(&self, hdl: &mut dyn Handler<S>) -> Result<(), Error> {
        if let Some(msg) = self.rgate.fetch() {
            let mut is = GateIStream::new(msg, &self.rgate);
            match self.handle_ctrl_msg(hdl, &mut is) {
                // should the server terminate?
                Ok(true) => return Err(Error::new(Code::EndOfFile)),
                // everything okay
                Ok(_) => {},
                // error, reply error code
                Err(e) => {
                    llog!(SERV, "Control channel request failed: {:?}", e);
                    is.reply_error(e.code()).ok();
                },
            }
        }
        Ok(())
    }

    fn handle_ctrl_msg<S>(
        &self,
        hdl: &mut dyn Handler<S>,
        is: &mut GateIStream<'_>,
    ) -> Result<bool, Error> {
        let op: service::Operation = is.pop()?;
        match op {
            service::Operation::OPEN => Self::handle_open(hdl, self.sel(), is),
            service::Operation::DERIVE_CRT => Self::handle_derive_crt(hdl, is),
            service::Operation::OBTAIN => Self::handle_obtain(hdl, is),
            service::Operation::DELEGATE => Self::handle_delegate(hdl, is),
            service::Operation::CLOSE => Self::handle_close(hdl, is),
            service::Operation::SHUTDOWN => match Self::handle_shutdown(hdl, is) {
                Ok(_) => return Ok(true),
                Err(e) => Err(e),
            },
            _ => is.reply_error(Code::InvArgs),
        }
        .map(|_| false)
    }

    fn handle_open<S>(
        hdl: &mut dyn Handler<S>,
        sel: Selector,
        is: &mut GateIStream<'_>,
    ) -> Result<(), Error> {
        let arg: &str = is.pop()?;

        let crt = is.label() as usize;
        let res = hdl.open(crt, sel, arg);

        llog!(SERV, "server::open(crt={}, arg={}) -> {:?}", crt, arg, res);

        match res {
            Ok((sel, ident)) => {
                let mut buf = MsgBuf::borrow_def();
                buf.set(service::OpenReply {
                    res: 0,
                    sess: sel,
                    ident: ident as u64,
                });
                is.reply(&buf)
            },
            Err(e) => {
                let mut buf = MsgBuf::borrow_def();
                buf.set(service::OpenReply {
                    res: e.code() as u64,
                    sess: 0,
                    ident: 0,
                });
                is.reply(&buf)
            },
        }
    }

    fn handle_derive_crt<S>(
        hdl: &mut dyn Handler<S>,
        is: &mut GateIStream<'_>,
    ) -> Result<(), Error> {
        let msg = is.msg().get_data::<service::DeriveCreator>();

        let crt = is.label() as usize;
        let sessions = msg.sessions as usize;

        llog!(
            SERV,
            "server::derive_crt(crt={}, sessions={})",
            crt,
            sessions
        );

        let (nid, sgate) = hdl.sessions().derive_creator(is.rgate(), crt, sessions)?;

        let mut buf = MsgBuf::borrow_def();
        buf.set(service::DeriveCreatorReply {
            res: 0,
            creator: nid as u64,
            sgate_sel: sgate as u64,
        });
        is.reply(&buf)
    }

    fn handle_obtain<S>(hdl: &mut dyn Handler<S>, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let msg = is.msg().get_data::<service::Exchange>();
        let sid = msg.sess as SessId;
        let crt = is.label() as usize;

        llog!(SERV, "server::obtain(crt={}, sid={})", crt, sid);

        if !hdl.sessions().creator_owns(crt, sid) {
            return Err(Error::new(Code::NoPerm));
        }

        let mut buf = MsgBuf::new();
        let reply = buf.set(service::ExchangeReply::default());

        let (res, args_size, crd) = {
            let mut xchg = CapExchange::new(&msg.data, &mut reply.data);

            let res = hdl.obtain(crt, sid, &mut xchg);

            llog!(
                SERV,
                "server::obtain(crt={}, sid={}) -> xchg={:?}), res={:?}",
                crt,
                sid,
                xchg,
                res
            );

            (res, xchg.out_args().size(), xchg.out_crd)
        };

        reply.res = match res {
            Ok(_) => 0,
            Err(e) => e.code() as u64,
        };
        reply.data.args.bytes = args_size as u64;
        reply.data.caps = crd.raw();
        is.reply(&buf)
    }

    fn handle_delegate<S>(hdl: &mut dyn Handler<S>, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let msg = is.msg().get_data::<service::Exchange>();
        let sid = msg.sess as SessId;
        let crt = is.label() as usize;

        llog!(SERV, "server::delegate(crt={}, sid={})", crt, sid);

        if !hdl.sessions().creator_owns(crt, sid) {
            return Err(Error::new(Code::NoPerm));
        }

        let mut buf = MsgBuf::new();
        let reply = buf.set(service::ExchangeReply::default());

        let (res, args_size, crd) = {
            let mut xchg = CapExchange::new(&msg.data, &mut reply.data);

            let res = hdl.delegate(crt, sid, &mut xchg);

            llog!(
                SERV,
                "server::delegate(crt={}, sid={}) -> xchg={:?}), res={:?}",
                crt,
                sid,
                xchg,
                res
            );

            (res, xchg.out_args().size(), xchg.out_crd)
        };

        reply.res = match res {
            Ok(_) => 0,
            Err(e) => e.code() as u64,
        };
        reply.data.args.bytes = args_size as u64;
        reply.data.caps = crd.raw();
        is.reply(&buf)
    }

    fn handle_close<S>(hdl: &mut dyn Handler<S>, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let sid = is.pop::<SessId>()?;
        let crt = is.label() as usize;

        llog!(SERV, "server::close(crt={}, sid={})", crt, sid);

        if !hdl.sessions().creator_owns(crt, sid) {
            return Err(Error::new(Code::NoPerm));
        }

        hdl.close(crt, sid as SessId);

        is.reply_error(Code::None)
    }

    fn handle_shutdown<S>(hdl: &mut dyn Handler<S>, is: &mut GateIStream<'_>) -> Result<(), Error> {
        llog!(SERV, "server::shutdown()");

        // only the first creator is allowed to shut us down
        let crt = is.label() as usize;
        if crt != 0 {
            return Err(Error::new(Code::NoPerm));
        }

        hdl.shutdown();

        is.reply_error(Code::None)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.public {
            Activity::own()
                .resmng()
                .unwrap()
                .unreg_service(self.sel())
                .ok();
        }
    }
}
