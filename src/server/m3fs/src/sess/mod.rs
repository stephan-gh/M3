/*
 * Copyright (C) 2015-2020, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

mod file_session;
mod meta_session;
mod open_files;

pub use file_session::FileSession;
pub use meta_session::MetaSession;
pub use open_files::OpenFiles;

use m3::com::GateIStream;
use m3::errors::{Code, Error};

pub enum FSSession {
    Meta(meta_session::MetaSession),
    File(file_session::FileSession),
}

impl FSSession {
    pub fn is_file_session(&self) -> bool {
        match self {
            FSSession::File(_) => true,
            _ => false,
        }
    }
}

impl M3FSSession for FSSession {
    fn creator(&self) -> usize {
        match self {
            FSSession::Meta(m) => m.creator(),
            FSSession::File(f) => f.creator(),
        }
    }

    fn next_in(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.next_in(stream),
            FSSession::File(f) => f.next_in(stream),
        }
    }

    fn next_out(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.next_out(stream),
            FSSession::File(f) => f.next_out(stream),
        }
    }

    fn commit(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.commit(stream),
            FSSession::File(f) => f.commit(stream),
        }
    }

    fn seek(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.seek(stream),
            FSSession::File(f) => f.seek(stream),
        }
    }

    fn fstat(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.fstat(stream),
            FSSession::File(f) => f.fstat(stream),
        }
    }

    fn stat(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.stat(stream),
            FSSession::File(f) => f.stat(stream),
        }
    }

    fn mkdir(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.mkdir(stream),
            FSSession::File(f) => f.mkdir(stream),
        }
    }

    fn rmdir(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.rmdir(stream),
            FSSession::File(f) => f.rmdir(stream),
        }
    }

    fn link(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.link(stream),
            FSSession::File(f) => f.link(stream),
        }
    }

    fn unlink(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.unlink(stream),
            FSSession::File(f) => f.unlink(stream),
        }
    }

    fn rename(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.rename(stream),
            FSSession::File(f) => f.rename(stream),
        }
    }

    fn sync(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.sync(stream),
            FSSession::File(f) => f.sync(stream),
        }
    }
}

/// Represents an abstract server-side M3FS Session.
pub trait M3FSSession {
    fn creator(&self) -> usize;

    fn next_in(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn next_out(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn commit(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn seek(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn fstat(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn stat(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn mkdir(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn rmdir(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn link(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn unlink(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn rename(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
    fn sync(&mut self, _stream: &mut GateIStream) -> Result<(), Error> {
        Err(Error::new(Code::NotSup))
    }
}
