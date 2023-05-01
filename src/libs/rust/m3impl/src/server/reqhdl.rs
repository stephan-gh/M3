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

use core::convert::TryFrom;
use core::fmt::Debug;
use core::marker::PhantomData;

use crate::boxed::Box;
use crate::cap::Selector;
use crate::cfg;
use crate::col::Vec;
use crate::com::{opcodes, GateIStream, RecvGate, SGateArgs, SendGate};
use crate::errors::{Code, Error};
use crate::format;
use crate::io::LogFlags;
use crate::kif;
use crate::log;
use crate::server::{server_loop, CapExchange, ExcType, Handler, Server, SessId, SessionContainer};
use crate::session::ServerSession;
use crate::tcu::Label;
use crate::tiles::Activity;
use crate::util::{self, math};
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
    /// The argument `serv` is the server session object, and `arg` is a string of arguments passed
    /// by the resource manager on behalf of the client.
    fn new(serv: ServerSession, arg: &str) -> Result<Self, Error>
    where
        Self: Sized;

    /// Returns the creator of this session
    fn creator(&self) -> usize;

    /// Returns whether this session is still alive
    ///
    /// This method will be used on remove "dead" sessions after each request handling. Therefore,
    /// overriding this method and setting a session to "dead" in a request handler allows to remove
    /// this session upon client requests. Note however that only the session that received the
    /// request is considered for removal!
    fn alive(&self) -> bool {
        true
    }

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

impl<S: RequestSession + 'static, O: Into<usize> + TryFrom<usize> + Debug> Handler<S, O>
    for RequestHandler<S, O>
{
    fn sessions(&mut self) -> &mut SessionContainer<S> {
        &mut self.clients.sessions
    }

    fn init(&mut self, serv: &Server) {
        self.clients.serv_sel = serv.sel();
    }

    fn exchange_handler(
        &mut self,
        crt: usize,
        sid: SessId,
        opcode: usize,
        ty: ExcType,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let Self {
            clients, cap_hdls, ..
        } = self;

        if opcode == opcodes::General::Connect.into() {
            clients.connect(crt, sid, ty, xchg)
        }
        else {
            match &cap_hdls[opcode] {
                CapHandler {
                    ty: hdl_ty,
                    func: Some(func),
                } if *hdl_ty == ty => (func)(clients, crt, sid, xchg),
                _ => Err(Error::new(Code::InvArgs)),
            }
        }
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        let sid = self.clients.sessions.next_id()?;
        if !self.clients.sessions.can_add(crt) {
            return Err(Error::new(Code::NoSpace));
        }

        let sel = Activity::own().alloc_sel();
        let serv = ServerSession::new_with_sel(srv_sel, sel, crt, sid, false)?;
        let sess = S::new(serv, arg)?;
        // the add cannot fail, because we called can_add before
        self.clients.sessions.add(crt, sid, sess).unwrap();
        Ok((sel, sid))
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
    serv_sel: Selector,
    sessions: SessionContainer<S>,
    rgate: RecvGate,
    sgates: Vec<(SessId, SendGate)>,
    max_cli_cons: usize,
}

impl<S: RequestSession + 'static> ClientManager<S> {
    /// Creates a new client manager for `max_clients` using a message size of `msg_size`.
    /// Additionally, `max_cli_cons` defines the maximum connections each client session may create.
    pub fn new(max_clients: usize, msg_size: usize, max_cli_cons: usize) -> Result<Self, Error> {
        let rgate = RecvGate::new(
            math::next_log2(max_clients * msg_size),
            math::next_log2(msg_size),
        )?;
        rgate.activate()?;
        Ok(Self {
            // will be initialized during the init call in the Handler trait
            serv_sel: 0,
            sessions: SessionContainer::new(max_clients),
            rgate,
            sgates: Vec::new(),
            max_cli_cons,
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

    /// Adds a new connection ([`SendGate`]) for the existing session with given id.
    ///
    /// Returns the selector of the [`SendGate`]
    pub fn add_connection(&mut self, sid: SessId) -> Result<Selector, Error> {
        // check if the client has already exceeded the connection limit
        let cons = self.sgates.iter().filter(|s| s.0 == sid).count();
        if cons + 1 > self.max_cli_cons {
            return Err(Error::new(Code::NoSpace));
        }

        let sgate = SendGate::new_with(SGateArgs::new(&self.rgate).label(sid as Label).credits(1))?;
        let sel = sgate.sel();
        self.sgates.push((sid, sgate));

        Ok(sel)
    }

    /// Creates and adds a new session using `create_sess`.
    ///
    /// The `create_sess` closure receives the created [`ServerSession`] instance and is expected
    /// to store it to keep the session alive.
    ///
    /// Returns the selector and session id of the session
    pub fn add_session<F>(
        &mut self,
        crt: usize,
        create_sess: F,
    ) -> Result<(Selector, SessId), Error>
    where
        F: FnOnce(&mut Self, ServerSession) -> Result<S, Error>,
    {
        let sid = self.sessions.next_id()?;
        if !self.sessions.can_add(crt) {
            return Err(Error::new(Code::NoSpace));
        }

        // always enable autoclose here, because this session was created manually and not via the
        // session-open call through the resource manager and kernel. For that reason, the resource
        // manager will also not close the session and therefore we want to know when the session
        // capability was revoked to remove the session.
        let serv = ServerSession::new(self.serv_sel, crt, sid, true)?;
        let sel = serv.sel();
        let sess = create_sess(self, serv)?;
        // the add cannot fail, because we called can_add before
        self.sessions.add(crt, sid, sess).unwrap();
        Ok((sel, sid))
    }

    /// Creates a new session using `create_sess` with a newly created [`SendGate`] that allows the
    /// session to send requests to us.
    ///
    /// The `create_sess` closure receives the created [`ServerSession`] instance and is expected to
    /// store it to keep the session alive. The closure also receives a reference to the created
    /// [`SendGate`] in case it's required.
    ///
    /// Note that it allocates two consecutive selectors for the session and the [`SendGate`]. The
    /// first one (for the session) is returned, together with the chosen session id.
    ///
    /// Returns the selector and session id of the session
    pub fn add_connected_session<F>(
        &mut self,
        crt: usize,
        create_sess: F,
    ) -> Result<(Selector, SessId), Error>
    where
        F: FnOnce(&mut Self, ServerSession, &SendGate) -> Result<S, Error>,
    {
        let sid = self.sessions.next_id()?;
        if !self.sessions.can_add(crt) {
            return Err(Error::new(Code::NoSpace));
        }

        let sels = Activity::own().alloc_sels(2);
        let sgate = SendGate::new_with(
            SGateArgs::new(&self.rgate)
                .label(sid as Label)
                .credits(1)
                .sel(sels + 1),
        )?;
        // autoclose enabled for the same reason as above
        let serv = ServerSession::new_with_sel(self.serv_sel, sels, crt, sid, true)?;
        let sess = create_sess(self, serv, &sgate)?;

        // the add cannot fail, because we called can_add before
        self.sessions.add(crt, sid, sess).unwrap();
        self.sgates.push((sid, sgate));

        Ok((sels, sid))
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
        ty: ExcType,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        if ty != ExcType::Obt(1) {
            return Err(Error::new(Code::InvArgs));
        }

        let sel = self.add_connection(sid)?;
        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 1));
        Ok(())
    }
}

type CapHandlerFunc<S> =
    dyn Fn(&mut ClientManager<S>, usize, SessId, &mut CapExchange<'_>) -> Result<(), Error>;

struct CapHandler<S> {
    ty: ExcType,
    func: Option<Box<CapHandlerFunc<S>>>,
}

/// A handler function for messages
pub type MsgHandlerFunc<S> = Option<Box<dyn Fn(&mut S, &mut GateIStream<'_>) -> Result<(), Error>>>;

/// Handles requests from clients
///
/// [`RequestHandler`] is one implementation for [`Handler`] that is suitable for the typical server:
/// clients send requests to the server, which are handled by the server. For that reason, the
/// server maintains a list of sessions to hold client-specific state, and uses a [`RecvGate`] to
/// receive client requests. Clients can obtain a [`SendGate`] to the [`RecvGate`] via the operation
/// [`Connect`](`opcodes::General::Connect`).
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
pub struct RequestHandler<S, O> {
    clients: ClientManager<S>,
    msg_hdls: Vec<MsgHandlerFunc<S>>,
    cap_hdls: Vec<CapHandler<S>>,
    _opcode: PhantomData<O>,
}

impl<S: RequestSession + 'static, O: Into<usize> + TryFrom<usize> + Debug> RequestHandler<S, O> {
    /// Creates a new request handler with default arguments
    pub fn new() -> Result<Self, Error> {
        Self::new_with(DEF_MAX_CLIENTS, DEF_MSG_SIZE, 1)
    }

    /// Creates a new request handler for `max_clients` using a message size of `msg_size`.
    /// Additionally, `max_cli_cons` defines the maximum connections each client session may create.
    pub fn new_with(
        max_clients: usize,
        msg_size: usize,
        max_cli_cons: usize,
    ) -> Result<Self, Error> {
        Ok(Self {
            clients: ClientManager::new(max_clients, msg_size, max_cli_cons)?,
            msg_hdls: Vec::new(),
            cap_hdls: Vec::new(),
            _opcode: PhantomData::default(),
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
    /// have the given opcode as the first integer. Furthermore, the exchange type (obtain or
    /// delegate) and the number of exchanged capabilities need to match.
    ///
    /// Note that `opcode` will be used as an index into a `Vec` and should therefore be reasonably
    /// small.
    pub fn reg_cap_handler<F>(&mut self, opcode: O, ty: ExcType, func: F)
    where
        F: Fn(&mut ClientManager<S>, usize, SessId, &mut CapExchange<'_>) -> Result<(), Error>
            + 'static,
    {
        let idx = opcode.into();
        while idx >= self.cap_hdls.len() {
            self.cap_hdls.push(CapHandler {
                ty: ExcType::Del(1),
                func: None,
            });
        }
        assert!(self.cap_hdls[idx].func.is_none());
        self.cap_hdls[idx] = CapHandler {
            ty,
            func: Some(Box::new(func)),
        };
    }

    /// Registers `func` as the message handler for the given opcode
    ///
    /// This function is called whenever `fetch_and_handle` is called and receives a message with
    /// this opcode as the first integer. The function is expected to reply to the caller unless
    /// there is an error. In the latter case, `fetch_and_handle` is responsible for replying with
    /// an error.
    ///
    /// Note that `opcode` will be used as an index into a `Vec` and should therefore be reasonably
    /// small.
    pub fn reg_msg_handler<F>(&mut self, opcode: O, func: F)
    where
        F: Fn(&mut S, &mut GateIStream<'_>) -> Result<(), Error> + 'static,
    {
        let idx = opcode.into();
        while idx >= self.msg_hdls.len() {
            self.msg_hdls.push(None);
        }
        assert!(self.msg_hdls[idx].is_none());
        self.msg_hdls[idx] = Some(Box::new(func));
    }

    /// Fetches the next message from the receive gate and calls the appropriate handler function
    /// depending on the opcode (first integer in the message).
    ///
    /// The handlers are registered via `reg_msg_handler`. In case the handler returns an error,
    /// this function sends a reply to the caller with the error code.
    pub fn fetch_and_handle(&mut self) -> Result<(), Error> {
        self.fetch_and_handle_with(|handler, opcode, sess, is| match &handler[opcode] {
            Some(f) => f(sess, is),
            None => Err(Error::new(Code::InvArgs)),
        })
    }

    /// Fetches the next message from the receive gate and calls the given function to handle it.
    pub fn fetch_and_handle_with<F>(&mut self, func: F) -> Result<(), Error>
    where
        F: FnOnce(
            &Vec<MsgHandlerFunc<S>>,
            usize,
            &mut S,
            &mut GateIStream<'_>,
        ) -> Result<(), Error>,
    {
        if let Ok(msg) = self.clients.rgate.fetch() {
            let mut is = GateIStream::new(msg, &self.clients.rgate);
            let opcode = is.pop::<usize>()?;
            let sid = is.label() as SessId;

            let op_name = |opcode| match O::try_from(opcode) {
                Ok(op) => format!("{:?}:{}", op, opcode),
                _ => format!("??:{}", opcode),
            };

            log!(
                LogFlags::LibServReqs,
                "server::request(sid={}, op={})",
                sid,
                op_name(opcode),
            );

            let sess = self.clients.sessions.get_mut(sid).unwrap();
            let res = func(&self.msg_hdls, opcode, sess, &mut is);

            log!(
                LogFlags::LibServReqs,
                "server::request(sid={}, op={}) -> {:?}",
                sid,
                op_name(opcode),
                res,
            );

            if let Err(e) = res {
                // ignore errors here
                is.reply_error(e.code()).ok();
            }

            if util::unlikely(!sess.alive()) {
                let crt = sess.creator();
                drop(sess);
                drop(is);
                self.clients.remove_session(crt, sid);
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
