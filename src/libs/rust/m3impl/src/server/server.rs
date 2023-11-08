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

use crate::cap::{CapFlags, Capability, SelSpace, Selector};
use crate::com::{GateIStream, RecvGate};
use crate::errors::{Code, Error};
use crate::io::LogFlags;
use crate::kif::{
    service::{DeriveCreatorReply, ExchangeData, ExchangeReply, OpenReply, Request},
    CapRngDesc,
};
use crate::log;
use crate::serialize::{M3Deserializer, M3Serializer, SliceSink};
use crate::server::{SessId, SessionContainer};
use crate::syscalls;
use crate::tiles::Activity;
use crate::util::math;

const MSG_SIZE: usize = 256;
const BUF_SIZE: usize = MSG_SIZE * (1 + super::sesscon::MAX_CREATORS);

/// Describes the type of capability exchange including the number of capabilities
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExcType {
    /// A delegate (client copies caps to the server)
    Del(u64),
    /// An obtain (server copies caps to the client)
    Obt(u64),
}

/// The struct to exchange capabilities with a client (obtain/delegate)
pub struct CapExchange<'d> {
    ty: ExcType,
    src: M3Deserializer<'d>,
    sink: M3Serializer<SliceSink<'d>>,
    pub(crate) out_crd: CapRngDesc,
}

impl<'d> CapExchange<'d> {
    /// Creates a new `CapExchange` object, taking input arguments from `input` and putting output
    /// arguments into `output`.
    pub fn new(ty: ExcType, input: &'d ExchangeData, output: &'d mut ExchangeData) -> Self {
        let len = (input.args.bytes + 7) / 8;
        Self {
            ty,
            src: M3Deserializer::new(&input.args.data[..len]),
            sink: M3Serializer::new(SliceSink::new(&mut output.args.data)),
            out_crd: CapRngDesc::default(),
        }
    }

    /// Returns the type of exchange including the number of capabilities
    pub fn ty(&self) -> ExcType {
        self.ty
    }

    /// Returns the input arguments
    pub fn in_args(&mut self) -> &mut M3Deserializer<'d> {
        &mut self.src
    }

    /// Returns the output arguments
    pub fn out_args(&mut self) -> &mut M3Serializer<SliceSink<'d>> {
        &mut self.sink
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
            "CapExchange[ty={:?}, out_crd={}]",
            self.ty, self.out_crd,
        )
    }
}

/// The handler customizes a [`Server`] by defining the opening and closing of sessions and capability
/// exchanges.
///
/// In more detail, for every request the [`Server`] receives from the kernel, it will call the
/// corresponding function of `Handler`. For example, whenever [`Server`] receives a open-session
/// request from the kernel, it will call [`Handler::open`] to let the `Handler` create the session
/// (if desired).
pub trait Handler<S> {
    /// Returns the session container
    fn sessions(&mut self) -> &mut SessionContainer<S>;

    /// Is called during initialization of the server and gives the handler a chance to perform
    /// further initialization based on the given server instance.
    fn init(&mut self, _serv: &Server) {
    }

    /// Opens a new session for a client
    ///
    /// This method is called by `Server` whenever a new session should be opened and is expected to
    /// create it. It receives the id of the session creator (`crt`), the selector of the server, and the
    /// arguments for the session. Note that the arguments are not specified by the calling
    /// application, but in the booted configuration file.
    ///
    /// Returns the used session selector and session identifier.
    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error>;

    /// Performs a capability exchange between a client and our service
    ///
    /// This method is called by `Server` whenever capabilities should be exchanged over an existing
    /// session. It receives the id of the session creator (`crt`), the id of the session, and the
    /// [`CapExchange`] data structure. The last argument contains information about the exchange
    /// (e.g., obtain/delegate) and is used to exchange input and output arguments with the client.
    fn exchange(
        &mut self,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error>;

    /// Closes the given session
    ///
    /// This method is called by `Server` whenever a session should be closed and receives the
    /// session creator (`crt`) and the session id as arguments.
    fn close(&mut self, _crt: usize, _sid: SessId) {
    }

    /// Shuts down the server
    ///
    /// This method is called by `Server` upon receiving the shutdown request from the kernel and
    /// allows handlers to performs cleanup actions before actually shutting down.
    fn shutdown(&mut self) {
    }
}

/// Represents a server that provides a service for clients.
///
/// The `Server` is the corner stone of the server API in M³, because it handles new connections
/// from clients, capability exchanges, and connection tear downs. These connections are represented
/// as sessions. That is, as soon as a client is connected to a server, the server uses a session to
/// represent this connection and potentially keep client-specific state.
///
/// How exactly sessions are opened, closed, and capabilities are exchanged is defined by the
/// [`Handler`], which is used by the server to handle the requests from the kernel. `Server`
/// therefore does not care what exactly a session is, but leaves this part to the [`Handler`].
pub struct Server {
    cap: Capability,
    rgate: RecvGate,
    public: bool,
}

impl Server {
    /// Creates a new server with given service name.
    pub fn new<H, S>(name: &str, hdl: &mut H) -> Result<Self, Error>
    where
        H: Handler<S>,
    {
        Self::create(name, hdl, true)
    }

    /// Creates a new private server that is not visible to anyone
    pub fn new_private<H, S>(name: &str, hdl: &mut H) -> Result<Self, Error>
    where
        H: Handler<S>,
    {
        Self::create(name, hdl, false)
    }

    fn create<H, S>(name: &str, hdl: &mut H, public: bool) -> Result<Self, Error>
    where
        H: Handler<S>,
    {
        let sel = SelSpace::get().alloc_sel();
        let rgate = RecvGate::new(math::next_log2(BUF_SIZE), math::next_log2(MSG_SIZE))?;
        rgate.activate()?;

        syscalls::create_srv(sel, rgate.sel(), name, 0)?;

        let max = hdl.sessions().capacity() as u32;
        let (_, sgate) = hdl.sessions().add_creator(&rgate, max)?;

        if public {
            Activity::own()
                .resmng()
                .unwrap()
                .reg_service(sel, sgate, name, max)?;
        }

        let serv = Server {
            cap: Capability::new(sel, CapFlags::empty()),
            rgate,
            public,
        };
        hdl.init(&serv);
        Ok(serv)
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
    ///
    /// Returns [`Code::EndOfFile`] if the server should shut down
    pub fn fetch_and_handle<H, S>(&self, hdl: &mut H) -> Result<(), Error>
    where
        H: Handler<S>,
    {
        if let Ok(msg) = self.rgate.fetch() {
            let mut is = GateIStream::new(msg, &self.rgate);
            match self.handle(hdl, &mut is) {
                // should the server terminate?
                Ok(true) => return Err(Error::new(Code::EndOfFile)),
                // everything okay
                Ok(_) => {},
                // error, reply error code
                Err(e) => {
                    log!(LogFlags::LibServ, "Control channel request failed: {:?}", e);
                    is.reply_error(e.code()).ok();
                },
            }
        }
        Ok(())
    }

    fn handle<H, S>(&self, hdl: &mut H, is: &mut GateIStream<'_>) -> Result<bool, Error>
    where
        H: Handler<S>,
    {
        let req: Request<'_> = is.pop()?;
        match req {
            Request::Open { arg } => Self::handle_open(hdl, self.sel(), is, arg),
            Request::DeriveCrt { sessions } => Self::handle_derive_crt(hdl, is, sessions),
            Request::Obtain { sid, data } => {
                self.handle_exchange(hdl, is, sid as SessId, &data, true)
            },
            Request::Delegate { sid, data } => {
                self.handle_exchange(hdl, is, sid as SessId, &data, false)
            },
            Request::Close { sid } => Self::handle_close(hdl, is, sid as SessId),
            Request::Shutdown => match Self::handle_shutdown(hdl, is) {
                Ok(_) => return Ok(true),
                Err(e) => Err(e),
            },
        }
        .map(|_| false)
    }

    fn handle_open<H, S>(
        hdl: &mut H,
        sel: Selector,
        is: &mut GateIStream<'_>,
        arg: &str,
    ) -> Result<(), Error>
    where
        H: Handler<S>,
    {
        let crt = is.label() as usize;
        let res = hdl.open(crt, sel, arg);

        log!(
            LogFlags::LibServ,
            "server::open(crt={}, arg={}) -> {:?}",
            crt,
            arg,
            res
        );

        match res {
            Ok((sel, ident)) => reply_vmsg!(is, Code::Success, OpenReply {
                sid: sel,
                ident: ident as u64,
            }),
            Err(e) => reply_vmsg!(is, e.code(), OpenReply { sid: 0, ident: 0 }),
        }
    }

    fn handle_derive_crt<H, S>(
        hdl: &mut H,
        is: &mut GateIStream<'_>,
        sessions: u32,
    ) -> Result<(), Error>
    where
        H: Handler<S>,
    {
        let crt = is.label() as usize;
        log!(
            LogFlags::LibServ,
            "server::derive_crt(crt={}, sessions={})",
            crt,
            sessions
        );

        let (nid, sgate) = hdl.sessions().derive_creator(is.rgate(), crt, sessions)?;

        reply_vmsg!(is, Code::Success, DeriveCreatorReply {
            creator: nid,
            sgate_sel: sgate,
        })
    }

    fn handle_exchange<H, S>(
        &self,
        hdl: &mut H,
        is: &mut GateIStream<'_>,
        sid: SessId,
        data: &ExchangeData,
        obtain: bool,
    ) -> Result<(), Error>
    where
        H: Handler<S>,
    {
        let crt = is.label() as usize;

        let mut reply = ExchangeReply::default();

        let ty = if obtain {
            ExcType::Obt(data.caps.count())
        }
        else {
            ExcType::Del(data.caps.count())
        };

        let (res, args_size, crd) = {
            let mut xchg = CapExchange::new(ty, data, &mut reply.data);

            let res = if !hdl.sessions().creator_owns(crt, sid) {
                Err(Error::new(Code::NoPerm))
            }
            else {
                hdl.exchange(crt, sid, &mut xchg)
            };

            (res, xchg.out_args().size(), xchg.out_crd)
        };

        let res = res.err().map(|e| e.code()).unwrap_or(Code::Success);
        reply.data.args.bytes = args_size;
        reply.data.caps = crd;
        reply_vmsg!(is, res, reply)
    }

    fn handle_close<H, S>(hdl: &mut H, is: &mut GateIStream<'_>, sid: SessId) -> Result<(), Error>
    where
        H: Handler<S>,
    {
        let crt = is.label() as usize;

        log!(LogFlags::LibServ, "server::close(crt={}, sid={})", crt, sid);

        if !hdl.sessions().creator_owns(crt, sid) {
            return Err(Error::new(Code::NoPerm));
        }

        hdl.close(crt, sid as SessId);

        is.reply_error(Code::Success)
    }

    fn handle_shutdown<H, S>(hdl: &mut H, is: &mut GateIStream<'_>) -> Result<(), Error>
    where
        H: Handler<S>,
    {
        log!(LogFlags::LibServ, "server::shutdown()");

        // only the first creator is allowed to shut us down
        let crt = is.label() as usize;
        if crt != 0 {
            return Err(Error::new(Code::NoPerm));
        }

        hdl.shutdown();

        is.reply_error(Code::Success)
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
