/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

mod file_session;
mod meta_session;
mod open_files;

pub use file_session::FileSession;
use meta_session::FileLimit;
pub use meta_session::MetaSession;
pub use open_files::OpenFiles;

use m3::col::Vec;
use m3::com::GateIStream;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::server::{CapExchange, ClientManager, RequestSession, ServerSession, SessId};
use m3::tiles::Activity;

#[allow(clippy::large_enum_variant)]
pub enum FSSession {
    Meta(meta_session::MetaSession),
    File(file_session::FileSession),
}

impl RequestSession for FSSession {
    fn new(serv: ServerSession, arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        // get max number of files
        let mut max_files: usize = 16;
        if arg.len() > 6 && &arg[..6] == "files=" {
            max_files = arg[6..].parse().map_err(|_| Error::new(Code::InvArgs))?;
        }

        log!(
            LogFlags::FSSess,
            "[{}] creating session(crt={}, max_files={})",
            serv.id(),
            serv.creator(),
            max_files
        );

        Ok(FSSession::Meta(MetaSession::new(
            serv,
            FileLimit::new(max_files),
        )))
    }

    fn close(&mut self, cli: &mut ClientManager<Self>, sid: SessId, sub_ids: &mut Vec<SessId>) {
        log!(
            LogFlags::FSSess,
            "[{}] fs::close(): closing {:?}",
            sid,
            sub_ids
        );

        match self {
            FSSession::Meta(ref meta) => {
                // remove contained file sessions
                sub_ids.extend_from_slice(meta.file_sessions());
            },

            FSSession::File(ref file) => {
                // remove file session from parent file session
                if let Some(psid) = file.parent_sess() {
                    if let Some(parent_file_session) = cli.get_mut(psid) {
                        match parent_file_session {
                            FSSession::File(ref mut pfs) => pfs.remove_child(sid),
                            _ => panic!("Parent FileSession is not a FileSession!?"),
                        }
                    }
                }
                // otherwise remove file session from parent meta session
                else if let Some(parent_meta_session) = cli.get_mut(file.meta_sess()) {
                    match parent_meta_session {
                        FSSession::Meta(ref mut pms) => pms.remove_file(sid),
                        _ => panic!("FileSession's parent is not a MetaSession!?"),
                    }
                }

                // remove child file sessions
                sub_ids.extend_from_slice(file.child_sessions());
            },
        }
    }
}

impl FSSession {
    fn get_sess(cli: &mut ClientManager<Self>, sid: SessId) -> Result<&mut Self, Error> {
        cli.get_mut(sid).ok_or_else(|| Error::new(Code::InvArgs))
    }

    pub fn open(
        cli: &mut ClientManager<Self>,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        cli.add_connected(crt, |cli, serv, _sgate| match Self::get_sess(cli, sid)? {
            FSSession::Meta(meta) => meta.open_file(serv, xchg).map(FSSession::File),
            _ => Err(Error::new(Code::InvArgs)),
        })
        .map(|_| ())
    }

    pub fn get_mem(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        match Self::get_sess(cli, sid)? {
            FSSession::File(file) => file.get_mem(xchg),
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    pub fn del_ep(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        match Self::get_sess(cli, sid)? {
            FSSession::Meta(m) => {
                let new_sel = Activity::own().alloc_sel();
                let id = m.add_ep(new_sel);
                log!(
                    LogFlags::FSSess,
                    "[{}] fs::add_ep(sel={}) -> {}",
                    sid,
                    new_sel,
                    id
                );
                xchg.out_caps(m3::kif::CapRngDesc::new(
                    m3::kif::CapType::Object,
                    new_sel,
                    1,
                ));
                xchg.out_args().push(id);
                Ok(())
            },
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    pub fn clone(
        cli: &mut ClientManager<Self>,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        cli.add_connected(crt, |cli, serv, _sgate| match Self::get_sess(cli, sid)? {
            FSSession::File(file) => file.clone(serv, xchg).map(FSSession::File),
            FSSession::Meta(meta) => meta.clone(serv, xchg).map(FSSession::Meta),
        })
        .map(|_| ())
    }

    pub fn set_dest(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        match Self::get_sess(cli, sid)? {
            FSSession::File(fs) => {
                let new_sel = Activity::own().alloc_sel();
                log!(LogFlags::FSSess, "[{}] fs::set_dest(sel={})", sid, new_sel);
                fs.set_ep(new_sel);
                xchg.out_caps(m3::kif::CapRngDesc::new(
                    m3::kif::CapType::Object,
                    new_sel,
                    1,
                ));
                Ok(())
            },
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    pub fn enable_notify(
        _cli: &mut ClientManager<Self>,
        _crt: usize,
        _sid: SessId,
        _xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
}

impl M3FSSession for FSSession {
    fn next_in(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.next_in(stream),
            FSSession::File(f) => f.next_in(stream),
        }
    }

    fn next_out(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.next_out(stream),
            FSSession::File(f) => f.next_out(stream),
        }
    }

    fn commit(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.commit(stream),
            FSSession::File(f) => f.commit(stream),
        }
    }

    fn seek(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.seek(stream),
            FSSession::File(f) => f.seek(stream),
        }
    }

    fn fstat(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.fstat(stream),
            FSSession::File(f) => f.fstat(stream),
        }
    }

    fn stat(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.stat(stream),
            FSSession::File(f) => f.stat(stream),
        }
    }

    fn get_path(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.get_path(stream),
            FSSession::File(f) => f.get_path(stream),
        }
    }

    fn truncate(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.truncate(stream),
            FSSession::File(f) => f.truncate(stream),
        }
    }

    fn mkdir(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.mkdir(stream),
            FSSession::File(f) => f.mkdir(stream),
        }
    }

    fn rmdir(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.rmdir(stream),
            FSSession::File(f) => f.rmdir(stream),
        }
    }

    fn link(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.link(stream),
            FSSession::File(f) => f.link(stream),
        }
    }

    fn unlink(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.unlink(stream),
            FSSession::File(f) => f.unlink(stream),
        }
    }

    fn rename(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.rename(stream),
            FSSession::File(f) => f.rename(stream),
        }
    }

    fn sync(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.sync(stream),
            FSSession::File(f) => f.sync(stream),
        }
    }

    fn open_priv(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.open_priv(stream),
            FSSession::File(f) => f.open_priv(stream),
        }
    }

    fn close_priv(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.close_priv(stream),
            FSSession::File(f) => f.close_priv(stream),
        }
    }
}

/// Represents an abstract server-side M3FS Session.
pub trait M3FSSession {
    fn next_in(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn next_out(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn commit(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn seek(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn fstat(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn stat(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn get_path(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn truncate(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn mkdir(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn rmdir(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn link(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn unlink(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn rename(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn sync(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn open_priv(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
    fn close_priv(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error>;
}
