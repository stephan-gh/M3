/*
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

use m3::col::Vec;
use m3::com::GateIStream;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif;
use m3::log;
use m3::server::{CapExchange, ClientManager, RequestSession, ServerSession, SessId};
use m3::tiles::Activity;

use crate::chan::{ChanType, Channel};
use crate::meta::Meta;
use crate::pipe::Pipe;

pub enum SessionData {
    Meta(Meta),
    Pipe(Pipe),
    Chan(Channel),
}

pub struct PipesSession {
    serv: ServerSession,
    data: SessionData,
}

impl PipesSession {
    pub fn new(serv: ServerSession, data: SessionData) -> Self {
        let res = PipesSession { serv, data };

        log!(
            LogFlags::PipeReqs,
            "[{}] pipes::new_{}(sel={})",
            res.serv.id(),
            match &res.data {
                SessionData::Meta(_) => "meta",
                SessionData::Pipe(_) => "pipe",
                SessionData::Chan(_) => "chan",
            },
            res.serv.sel(),
        );

        res
    }

    pub fn data(&self) -> &SessionData {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut SessionData {
        &mut self.data
    }
}

impl RequestSession for PipesSession {
    fn new(serv: ServerSession, _arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(PipesSession::new(serv, SessionData::Meta(Meta::default())))
    }

    fn close(&mut self, cli: &mut ClientManager<Self>, sid: SessId, sub_ids: &mut Vec<SessId>)
    where
        Self: Sized,
    {
        log!(
            LogFlags::PipeReqs,
            "[{}] pipes::close(): closing {:?}",
            sid,
            sub_ids
        );

        // ignore errors here
        let _ = match &mut self.data_mut() {
            SessionData::Meta(ref mut m) => m.close(sub_ids),
            SessionData::Pipe(ref mut p) => p.close(sub_ids),
            SessionData::Chan(ref mut c) => c.close(sub_ids, cli.recv_gate()),
        };
    }
}

impl PipesSession {
    fn get_sess(cli: &mut ClientManager<Self>, sid: SessId) -> Result<&mut Self, Error> {
        cli.get_mut(sid).ok_or_else(|| Error::new(Code::InvArgs))
    }

    pub fn with_chan<F, R>(&mut self, is: &mut GateIStream<'_>, func: F) -> Result<R, Error>
    where
        F: Fn(&mut Channel, &mut GateIStream<'_>) -> Result<R, Error>,
    {
        match &mut self.data_mut() {
            SessionData::Chan(c) => func(c, is),
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    pub fn open_pipe(
        cli: &mut ClientManager<Self>,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let msize: usize = xchg.in_args().pop()?;

        log!(
            LogFlags::PipeReqs,
            "[{}] pipes::open_pipe(size={:#x})",
            sid,
            msize
        );

        let (sel, _nsid) = cli.add(crt, |cli, serv| {
            let parent_sess = Self::get_sess(cli, sid)?;

            match parent_sess.data_mut() {
                SessionData::Meta(ref mut m) => {
                    let pipe = m.create_pipe(serv.id(), msize);
                    Ok(PipesSession::new(serv, SessionData::Pipe(pipe)))
                },

                _ => Err(Error::new(Code::InvArgs)),
            }
        })?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 1));

        Ok(())
    }

    pub fn open_chan(
        cli: &mut ClientManager<Self>,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let ty = match xchg.in_args().pop()? {
            1 => ChanType::READ,
            _ => ChanType::WRITE,
        };

        log!(
            LogFlags::PipeReqs,
            "[{}] pipes::open_chan(ty={:?})",
            sid,
            ty
        );

        let (sel, _nsid) = cli.add_connected(crt, |cli, serv, _sgate| {
            let parent_sess = Self::get_sess(cli, sid)?;

            match parent_sess.data_mut() {
                SessionData::Pipe(ref mut p) => {
                    let chan = p.new_chan(serv.id(), ty)?;
                    p.attach(&chan);
                    Ok(PipesSession::new(serv, SessionData::Chan(chan)))
                },

                _ => Err(Error::new(Code::InvArgs)),
            }
        })?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 2));

        Ok(())
    }

    pub fn clone(
        cli: &mut ClientManager<Self>,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(LogFlags::PipeReqs, "[{}] pipes::clone()", sid,);

        let (sel, _nsid) = cli.add_connected(crt, |cli, serv, _sgate| {
            let parent_sess = Self::get_sess(cli, sid)?;

            let res = match &mut parent_sess.data_mut() {
                SessionData::Chan(ref mut c) => {
                    let chan = c.clone(serv.id())?;

                    Ok(PipesSession::new(serv, SessionData::Chan(chan)))
                },

                _ => Err(Error::new(Code::InvArgs)),
            }?;

            if let SessionData::Chan(ref c) = res.data() {
                let psess = Self::get_sess(cli, c.pipe()).unwrap();
                if let SessionData::Pipe(ref mut p) = psess.data_mut() {
                    p.attach(c);
                }
            }

            Ok(res)
        })?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 2));

        Ok(())
    }

    pub fn set_mem(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let sess = Self::get_sess(cli, sid)?;
        match &mut sess.data_mut() {
            SessionData::Pipe(ref mut p) => {
                if p.has_mem() {
                    return Err(Error::new(Code::InvArgs));
                }

                let sel = Activity::own().alloc_sel();
                p.set_mem(sel);

                log!(LogFlags::PipeReqs, "[{}] pipes::set_mem(sel={})", sid, sel);

                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 1));

                Ok(())
            },
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    pub fn set_dest(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let sess = Self::get_sess(cli, sid)?;
        match &mut sess.data_mut() {
            SessionData::Chan(ref mut c) => {
                let sel = Activity::own().alloc_sel();
                c.set_ep(sel);

                log!(LogFlags::PipeReqs, "[{}] pipes::set_dest(sel={})", sid, sel);

                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 1));

                Ok(())
            },

            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    pub fn enable_notify(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let sess = Self::get_sess(cli, sid)?;
        match &mut sess.data_mut() {
            SessionData::Chan(ref mut c) => {
                let sel = Activity::own().alloc_sel();
                log!(
                    LogFlags::PipeReqs,
                    "[{}] pipes::enable_notify(sel={})",
                    sid,
                    sel
                );
                c.enable_notify(sel)?;

                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 1));

                Ok(())
            },

            _ => Err(Error::new(Code::InvArgs)),
        }
    }
}
