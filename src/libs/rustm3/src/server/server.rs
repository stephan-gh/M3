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

use cap::{CapFlags, Capability, Selector};
use com::{GateIStream, RecvGate};
use dtu::EpId;
use errors::{Code, Error};
use kif::service;
use math;
use pes::VPE;
use server::SessId;
use syscalls;

/// Represents a server that provides a service for clients.
pub struct Server {
    cap: Capability,
    rgate: RecvGate,
    public: bool,
}

/// The handler for a server that implements the service calls (session creations, cap exchange,
/// ...).
pub trait Handler {
    /// Creates a new session with `arg` as an argument for the service with selector `srv_sel`.
    /// Returns the session selector and the session identifier.
    fn open(&mut self, srv_sel: Selector, arg: &str) -> Result<(Selector, SessId), Error>;

    /// Let's the client obtain a capability from the server
    fn obtain(&mut self, _sid: SessId, _data: &mut service::ExchangeData) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    /// Let's the client delegate a capability to the server
    fn delegate(&mut self, _sid: SessId, _data: &mut service::ExchangeData) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }

    /// Closes the given session
    fn close(&mut self, _sid: SessId) {
    }

    /// Performs cleanup actions before shutdown
    fn shutdown(&mut self) {
    }
}

const MSG_SIZE: usize = 256;
const BUF_SIZE: usize = MSG_SIZE * 2;

impl Server {
    /// Creates a new server with given service name.
    pub fn new(name: &str) -> Result<Self, Error> {
        Self::create(name, true)
    }

    /// Creates a new private server that is not visible to anyone
    pub fn new_private(name: &str) -> Result<Self, Error> {
        Self::create(name, false)
    }

    fn create(name: &str, public: bool) -> Result<Self, Error> {
        let sel = VPE::cur().alloc_sel();
        let mut rgate = RecvGate::new(math::next_log2(BUF_SIZE), math::next_log2(MSG_SIZE))?;
        rgate.activate()?;

        if public {
            VPE::cur().resmng().reg_service(sel, rgate.sel(), name)?;
        }
        else {
            syscalls::create_srv(sel, VPE::cur().sel(), rgate.sel(), name)?;
        }

        Ok(Server {
            cap: Capability::new(sel, CapFlags::empty()),
            rgate,
            public,
        })
    }

    /// Binds a new server to given selector and receive EP.
    pub fn new_bind(caps: Selector, ep: EpId) -> Self {
        let mut rgate = RecvGate::new_bind(
            caps + 1,
            math::next_log2(BUF_SIZE),
            math::next_log2(MSG_SIZE),
        );
        rgate.set_ep(ep);

        Server {
            cap: Capability::new(caps + 0, CapFlags::KEEP_CAP),
            rgate,
            public: false,
        }
    }

    /// Returns the capability selector of the service
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Fetches a message from the control channel and handles it if so.
    pub fn handle_ctrl_chan(&self, hdl: &mut dyn Handler) -> Result<(), Error> {
        let is = self.rgate.fetch();
        if let Some(mut is) = is {
            let op: service::Operation = is.pop();
            match op {
                service::Operation::OPEN => Self::handle_open(hdl, self.sel(), is),
                service::Operation::OBTAIN => Self::handle_obtain(hdl, is),
                service::Operation::DELEGATE => Self::handle_delegate(hdl, is),
                service::Operation::CLOSE => Self::handle_close(hdl, is),
                service::Operation::SHUTDOWN => Self::handle_shutdown(hdl, is),
                _ => unreachable!(),
            }
        }
        else {
            Ok(())
        }
    }

    fn handle_open(hdl: &mut dyn Handler, sel: Selector, mut is: GateIStream) -> Result<(), Error> {
        let arg: &str = is.pop();
        let res = hdl.open(sel, arg);

        log!(SERV, "server::open({}) -> {:?}", arg, res);

        match res {
            Ok((sel, ident)) => {
                let reply = service::OpenReply {
                    res: 0,
                    sess: u64::from(sel),
                    ident: ident as u64,
                };
                is.reply(&[reply])?
            },
            Err(e) => {
                let reply = service::OpenReply {
                    res: e.code() as u64,
                    sess: 0,
                    ident: 0,
                };
                is.reply(&[reply])?
            },
        };
        Ok(())
    }

    fn handle_obtain(hdl: &mut dyn Handler, mut is: GateIStream) -> Result<(), Error> {
        let sid: u64 = is.pop();
        let mut data: service::ExchangeData = is.pop();
        let res = hdl.obtain(sid as SessId, &mut data);

        log!(SERV, "server::obtain({}, {:?}) -> {:?}", sid, data, res);

        let reply = service::ExchangeReply {
            res: match res {
                Ok(_) => 0,
                Err(e) => e.code() as u64,
            },
            data,
        };
        is.reply(&[reply])
    }

    fn handle_delegate(hdl: &mut dyn Handler, mut is: GateIStream) -> Result<(), Error> {
        let sid: u64 = is.pop();
        let mut data: service::ExchangeData = is.pop();
        let res = hdl.delegate(sid as SessId, &mut data);

        log!(SERV, "server::delegate({}, {:?}) -> {:?}", sid, data, res);

        let reply = service::ExchangeReply {
            res: match res {
                Ok(_) => 0,
                Err(e) => e.code() as u64,
            },
            data,
        };
        is.reply(&[reply])
    }

    fn handle_close(hdl: &mut dyn Handler, mut is: GateIStream) -> Result<(), Error> {
        let sid: u64 = is.pop();

        log!(SERV, "server::close({})", sid);

        hdl.close(sid as SessId);

        reply_vmsg!(is, 0)
    }

    fn handle_shutdown(hdl: &mut dyn Handler, mut is: GateIStream) -> Result<(), Error> {
        log!(SERV, "server::shutdown()");

        hdl.shutdown();

        reply_vmsg!(is, 0)?;
        Err(Error::new(Code::EndOfFile))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.public && !self.cap.flags().contains(CapFlags::KEEP_CAP) {
            VPE::cur().resmng().unreg_service(self.sel(), false).ok();
            self.cap.set_flags(CapFlags::KEEP_CAP);
        }
    }
}
