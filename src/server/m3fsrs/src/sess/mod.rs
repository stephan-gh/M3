mod file_session;
mod meta_session;
mod open_files;

pub use file_session::FileSession;
pub use meta_session::MetaSession;
pub use open_files::OpenFiles;

use m3::cell::RefCell;
use m3::com::GateIStream;
use m3::errors::{Code, Error};
use m3::rc::Rc;

pub enum FSSession {
    Meta(meta_session::MetaSession),
    File(Rc<RefCell<file_session::FileSession>>),
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
            FSSession::File(f) => f.borrow().creator(),
        }
    }

    fn next_in(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.next_in(stream),
            FSSession::File(f) => f.borrow_mut().next_in(stream),
        }
    }

    fn next_out(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.next_out(stream),
            FSSession::File(f) => f.borrow_mut().next_out(stream),
        }
    }

    fn commit(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.commit(stream),
            FSSession::File(f) => f.borrow_mut().commit(stream),
        }
    }

    fn seek(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.seek(stream),
            FSSession::File(f) => f.borrow_mut().seek(stream),
        }
    }

    fn fstat(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.fstat(stream),
            FSSession::File(f) => f.borrow_mut().fstat(stream),
        }
    }

    fn stat(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.stat(stream),
            FSSession::File(f) => f.borrow_mut().stat(stream),
        }
    }

    fn mkdir(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.mkdir(stream),
            FSSession::File(f) => f.borrow_mut().mkdir(stream),
        }
    }

    fn rmdir(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.rmdir(stream),
            FSSession::File(f) => f.borrow_mut().rmdir(stream),
        }
    }

    fn link(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.link(stream),
            FSSession::File(f) => f.borrow_mut().link(stream),
        }
    }

    fn unlink(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        match self {
            FSSession::Meta(m) => m.unlink(stream),
            FSSession::File(f) => f.borrow_mut().unlink(stream),
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
}
