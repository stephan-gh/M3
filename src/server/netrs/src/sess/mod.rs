/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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
use m3::com::GateIStream;
use m3::errors::{Code, Error};
use m3::server::CapExchange;

use smoltcp::socket::SocketSet;

pub mod file;
pub mod socket;

pub use file::FileSession;
pub use socket::SocketSession;

pub const MSG_SIZE: usize = 128;

pub enum NetworkSession {
    FileSession(FileSession),
    SocketSession(SocketSession),
}

impl NetworkSession {
    pub fn obtain(
        &mut self,
        crt: usize,
        server: Selector,
        xchg: &mut CapExchange,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.obtain(crt, server, xchg),
        }
    }

    pub fn delegate(&mut self, xchg: &mut CapExchange) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(fs) => fs.delegate(xchg),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn stat(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn seek(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn next_in(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn next_out(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn commit(&mut self, _is: &mut GateIStream) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(_ss) => Err(Error::new(Code::NotSup)),
        }
    }

    pub fn create(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.create(is, socket_set),
        }
    }

    pub fn bind(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.bind(is, socket_set),
        }
    }

    pub fn listen(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.listen(is, socket_set),
        }
    }

    pub fn connect(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.connect(is, socket_set),
        }
    }

    pub fn abort(
        &mut self,
        is: &mut GateIStream,
        socket_set: &mut SocketSet<'static>,
    ) -> Result<(), Error> {
        match self {
            NetworkSession::FileSession(_fs) => Err(Error::new(Code::NotSup)),
            NetworkSession::SocketSession(ss) => ss.abort(is, socket_set),
        }
    }
}
