/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

use crate::boxed::Box;
use crate::cap::Selector;
use crate::cfg;
use crate::col::Vec;
use crate::com::{opcodes, GateIStream, RecvGate, SGateArgs, SendGate};
use crate::errors::{Code, Error};
use crate::kif;
use crate::server::{server_loop, CapExchange, ExcType, Handler, Server, SessId, SessionContainer};
use crate::session::ServerSession;
use crate::tcu::Label;
use crate::util::math;
use crate::vec;

/// The default maximum number of clients a service supports
pub const DEF_MAX_CLIENTS: usize = if cfg::MAX_ACTS < 32 {
    cfg::MAX_ACTS
}
else {
    32
};

/// The default message size used for the requests
pub const DEF_MSG_SIZE: usize = 64;

/// Represents a session that can be used for request handling from clients
///
/// The [`RequestHandler`] therefore requires that sessions implement this trait and will call
/// [`RequestSession::new`] to create the session object and [`RequestSession::close`] to remove the
/// session object.
pub trait RequestSession {
    /// Creates a new instance of the session with given arguments.
    ///
    /// The argument `crt` specifies the creator, `serv` is the server session object, and `arg`
    /// is a string of arguments passed by the resource manager on behalf of the client.
    fn new(crt: usize, serv: ServerSession, arg: &str) -> Result<Self, Error>
    where
        Self: Sized;

    /// This method is called after the session has been removed from the session container and
    /// gives the session a chance to perform cleanup actions (with the [`RequestHandler`]).
    ///
    /// The argument `sid` specifies the id of the removed session, whereas `sub_ids` is a Vec of
    /// other session ids that are about to be removed. The `close` implementation can add more
    /// session ids (e.g., sub sessions) to the vector to close them as well.
    fn close(&mut self, _cli: &mut ClientManager<Self>, _sid: SessId, _sub_ids: &mut Vec<SessId>)
    where
        Self: Sized,
    {
    }
}

impl<S: RequestSession + 'static> Handler<S> for RequestHandler<S> {
    fn sessions(&mut self) -> &mut SessionContainer<S> {
        &mut self.clients.sessions
    }

    fn exchange_handler(
        &mut self,
        crt: usize,
        sid: SessId,
        opcode: u64,
        ty: ExcType,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let Self {
            clients, cap_hdls, ..
        } = self;

        if opcode == opcodes::General::CONNECT.val {
            clients.connect(crt, sid, xchg)
        }
        else {
            let cap_hdl = cap_hdls
                .iter()
                .find(|h| h.opcode == opcode && h.ty == ty)
                .ok_or_else(|| Error::new(Code::InvArgs))?;
            (cap_hdl.func)(clients, crt, sid, xchg)
        }
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        self.clients
            .sessions
            .add_next(crt, srv_sel, false, |serv| S::new(crt, serv, arg))
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        self.clients.remove_session(crt, sid);
    }
}

/// The client manager holds all sessions and the connections to clients
///
/// The sessions are stored via the [`SessionContainer`] and the connections are represented as a
/// [`RecvGate`] that clients can send to and a list of [`SendGate`]s.
///
/// [`ClientManager`] is used internally in [`RequestHandler`] and therefore does not need to be
/// created manually. However, some methods (e.g., capability exchange handlers) receive a reference
/// to the [`ClientManager`] to have access to all sessions etc.
pub struct ClientManager<S> {
    sessions: SessionContainer<S>,
    rgate: RecvGate,
    sgates: Vec<(SessId, SendGate)>,
}

impl<S: RequestSession + 'static> ClientManager<S> {
    /// Creates a new client manager for `max_clients` using a message size of `msg_size`.
    pub fn new(max_clients: usize, msg_size: usize) -> Result<Self, Error> {
        let rgate = RecvGate::new(
            math::next_log2(max_clients * msg_size),
            math::next_log2(msg_size),
        )?;
        rgate.activate()?;
        Ok(Self {
            sessions: SessionContainer::new(max_clients),
            rgate,
            sgates: Vec::new(),
        })
    }

    /// Returns the receive gate that is used to receive requests from clients
    pub fn recv_gate(&self) -> &RecvGate {
        &self.rgate
    }

    /// Returns a reference to the session container
    pub fn sessions(&self) -> &SessionContainer<S> {
        &self.sessions
    }

    /// Returns a mutable reference to the session container
    pub fn sessions_mut(&mut self) -> &mut SessionContainer<S> {
        &mut self.sessions
    }

    /// Adds a new connection ([`SendGate`]) for the given session id.
    ///
    /// Returns the selector of the [`SendGate`]
    pub fn add_connection(&mut self, sid: SessId) -> Result<Selector, Error> {
        let sgate = SendGate::new_with(SGateArgs::new(&self.rgate).label(sid as Label).credits(1))?;
        let sel = sgate.sel();
        self.sgates.push((sid, sgate));
        Ok(sel)
    }

    /// Creates a new session using `create_sess` using a newly created [`SendGate`] that allows the
    /// session to send requests to us. The argument `sel` specifies the desired selector for the
    /// [`SendGate`].
    pub fn add_connected_session<F>(
        &mut self,
        crt: usize,
        sel: Selector,
        create_sess: F,
    ) -> Result<SessId, Error>
    where
        F: FnOnce(&mut Self, SessId, &SendGate) -> Result<S, Error>,
    {
        let sid = self.sessions.next_id()?;
        if !self.sessions.can_add(crt) {
            return Err(Error::new(Code::NoSpace));
        }

        let sgate = SendGate::new_with(
            SGateArgs::new(&self.rgate)
                .label(sid as Label)
                .credits(1)
                .sel(sel),
        )?;
        let sess = create_sess(self, sid, &sgate)?;

        // the add cannot fail, because we called can_add before
        self.sessions.add(crt, sid, sess).unwrap();
        self.sgates.push((sid, sgate));

        Ok(sid)
    }

    /// Retrieves the session with given id and calls the given function with that session.
    ///
    /// The function also receives the internal [`RecvGate`] in case it's needed.
    pub fn with_session<F, R>(&mut self, sid: SessId, mut func: F) -> Result<R, Error>
    where
        F: FnMut(&mut S, &RecvGate) -> Result<R, Error>,
    {
        let sess = self
            .sessions
            .get_mut(sid)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        func(sess, &self.rgate)
    }

    /// Removes the session with given id
    ///
    /// The removal calls `close` on the session, which has the option to add other sessions to the
    /// removal.
    pub fn remove_session(&mut self, crt: usize, sid: SessId) {
        self.sgates.retain(|s| s.0 != sid);

        // close this and all child sessions
        let mut sids = vec![sid];
        while let Some(id) = sids.pop() {
            if let Some(mut sess) = self.sessions.remove(crt, id) {
                sess.close(self, id, &mut sids);

                // ignore all potentially outstanding messages of this session
                self.recv_gate().drop_msgs_with(id as Label).unwrap();
            }
        }
    }

    fn connect(
        &mut self,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let sel = self.add_connection(sid)?;
        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }
}

struct CapHandler<S> {
    opcode: u64,
    ty: ExcType,
    func: Box<
        dyn Fn(&mut ClientManager<S>, usize, SessId, &mut CapExchange<'_>) -> Result<(), Error>,
    >,
}

pub struct MsgHandler<S> {
    opcode: u64,
    func: Box<dyn Fn(&mut S, &mut GateIStream<'_>) -> Result<(), Error>>,
}

impl<S> MsgHandler<S> {
    /// Returns the opcode this handler is responsible for
    pub fn opcode(&self) -> u64 {
        self.opcode
    }

    /// Returns the handler function
    pub fn func(&self) -> &Box<dyn Fn(&mut S, &mut GateIStream<'_>) -> Result<(), Error>> {
        &self.func
    }
}

/// Handles requests from clients
///
/// [`RequestHandler`] is one implementation for [`Handler`] that is suitable for the typical server:
/// clients send requests to the server, which are handled by the server. For that reason, the
/// server maintains a list of sessions to hold client-specific state, and uses a [`RecvGate`] to
/// receive client requests. Clients can obtain a [`SendGate`] to the [`RecvGate`] via the operation
/// [`CONNECT`](`opcodes::General::CONNECT`).
///
/// The actual requests are implemented by handler functions. [`RequestHandler`] supports both
/// capability handlers and message handlers. The former are called whenever a capability exchange
/// is desired by the client, whereas the latter are called whenever a request is sent over the
/// clients [`SendGate`]. Capability handlers and message handlers can be registered via
/// [`reg_cap_handler`](`RequestHandler::reg_cap_handler`) and
/// [`reg_msg_handler`](`RequestHandler::reg_msg_handler`), respectively.
///
/// The sessions are managed by the [`ClientManager`], which holds the client-specific data
/// (sessions) and their communication channels.
pub struct RequestHandler<S> {
    clients: ClientManager<S>,
    msg_hdls: Vec<MsgHandler<S>>,
    cap_hdls: Vec<CapHandler<S>>,
}

impl<S: RequestSession + 'static> RequestHandler<S> {
    /// Creates a new request handler with default arguments
    pub fn new() -> Result<Self, Error> {
        Self::new_with(DEF_MAX_CLIENTS, DEF_MSG_SIZE)
    }

    /// Creates a new request handler for `max_clients` using a message size of `msg_size`.
    pub fn new_with(max_clients: usize, msg_size: usize) -> Result<Self, Error> {
        Ok(Self {
            clients: ClientManager::new(max_clients, msg_size)?,
            msg_hdls: Vec::new(),
            cap_hdls: Vec::new(),
        })
    }

    /// Returns a reference to the client manager
    pub fn clients(&self) -> &ClientManager<S> {
        &self.clients
    }

    /// Returns a mutable reference to the client manager
    pub fn clients_mut(&mut self) -> &mut ClientManager<S> {
        &mut self.clients
    }

    /// Registers `func` as the capability handler for given opcode and exchange type
    ///
    /// This function is called whenever a capability should be exchanged with a client and the
    /// given opcode and exchange type match. That is, the message from the client is expected to
    /// have the given opcode as the first 64-bit word. Furthermore, the exchange type (obtain or
    /// delegate) and the number of exchanged capabilities need to match.
    pub fn reg_cap_handler<F>(&mut self, opcode: u64, ty: ExcType, func: F)
    where
        F: Fn(&mut ClientManager<S>, usize, SessId, &mut CapExchange<'_>) -> Result<(), Error>
            + 'static,
    {
        self.cap_hdls.push(CapHandler {
            opcode,
            ty,
            func: Box::new(func),
        });
    }

    /// Registers `func` as the message handler for the given opcode
    ///
    /// This function is called whenever `fetch_and_handle` is called and receives a message with
    /// this opcode as the first 64-bit word. The function is expected to reply to the caller unless
    /// there is an error. In the latter case, `fetch_and_handle` is responsible for replying with
    /// an error.
    pub fn reg_msg_handler<F>(&mut self, opcode: u64, func: F)
    where
        F: Fn(&mut S, &mut GateIStream<'_>) -> Result<(), Error> + 'static,
    {
        self.msg_hdls.push(MsgHandler {
            opcode,
            func: Box::new(func),
        });
    }

    /// Fetches the next message from the receive gate and calls the appropriate handler function
    /// depending on the opcode (first 64-bit word in the message).
    ///
    /// The handlers are registered via `reg_msg_handler`. In case the handler returns an error,
    /// this function sends a reply to the caller with the error code.
    pub fn fetch_and_handle(&mut self) -> Result<(), Error> {
        self.fetch_and_handle_with(|handler, opcode, sess, is| {
            let msg_hdl = handler
                .iter()
                .find(|h| h.opcode == opcode)
                .ok_or_else(|| Error::new(Code::InvArgs))?;

            (msg_hdl.func)(sess, is)
        })
    }

    /// Fetches the next message from the receive gate and calls the given function to handle it.
    pub fn fetch_and_handle_with<F>(&mut self, func: F) -> Result<(), Error>
    where
        F: FnOnce(&Vec<MsgHandler<S>>, u64, &mut S, &mut GateIStream<'_>) -> Result<(), Error>,
    {
        if let Ok(msg) = self.clients.rgate.fetch() {
            let mut is = GateIStream::new(msg, &self.clients.rgate);
            let opcode: u64 = is.pop()?;

            let sess = self.clients.sessions.get_mut(is.label() as SessId).unwrap();
            if let Err(e) = func(&self.msg_hdls, opcode, sess, &mut is) {
                // ignore errors here
                is.reply_error(e.code()).ok();
            }
        }
        Ok(())
    }

    /// Runs the default server loop
    pub fn run(&mut self, srv: &mut Server) -> Result<(), Error> {
        let res = server_loop(|| {
            srv.fetch_and_handle(self)?;

            self.fetch_and_handle()
        });

        match res {
            Ok(_) => Ok(()),
            Err(e) => match e.code() {
                Code::EndOfFile => Ok(()),
                _ => Err(e),
            },
        }
    }
}
