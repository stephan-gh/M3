use crate::data::{Dirs, INodes};
use crate::internal::{FileMode, InodeNo, OpenFlags};
use crate::sess::{FileSession, M3FSSession};
use crate::FileInfo;

use m3::{
    cap::Selector,
    cell::RefCell,
    col::Vec,
    com::{GateIStream, SendGate},
    errors::{Code, Error},
    rc::Rc,
    serialize::Source,
    server::CapExchange,
    server::SessId,
    session::ServerSession,
};

pub struct MetaSession {
    #[allow(dead_code)]
    server_session: ServerSession,
    sgates: Vec<SendGate>,
    max_files: usize,
    files: Vec<Option<Rc<RefCell<FileSession>>>>,
    creator: usize,
    session_id: SessId,
}

impl MetaSession {
    pub fn new(
        server_session: ServerSession,
        session_id: SessId,
        crt: usize,
        max_files: usize,
    ) -> Self {
        MetaSession {
            server_session,
            sgates: Vec::new(),
            max_files,
            files: vec![None; max_files],
            creator: crt,
            session_id,
        }
    }

    pub fn get_sgate(&mut self, data: &mut CapExchange) -> Result<(), Error> {
        if data.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let sgate = SendGate::new(crate::REQHDL.recv_gate())?;
        let sgate_selector = sgate.sel();
        self.sgates.push(sgate);
        data.out_caps(m3::kif::CapRngDesc::new(
            m3::kif::CapType::OBJECT,
            sgate_selector,
            1,
        ));
        Ok(())
    }

    pub fn remove_file(&mut self, file_session: Rc<RefCell<FileSession>>) {
        for i in 0..self.max_files {
            if let Some(ifs) = &self.files[i] {
                if ifs.borrow().ino() == file_session.borrow().ino() {
                    self.files.remove(i);
                    break;
                }
            }
        }
    }

    /// Creates a file session based on this meta session for `file_session_id`.
    /// If successful returns a pointer to this session.
    pub fn open_file(
        &mut self,
        selector: Selector,
        crt: usize,
        data: &mut CapExchange,
        file_session_id: SessId,
    ) -> Result<Rc<RefCell<FileSession>>, Error> {
        let flags = OpenFlags::from_bits_truncate(data.in_args().pop::<u64>()?);

        // Read the string, is already read only until termination or not at all
        let path = data.in_args().pop_str_slice()?;

        log!(
            crate::LOG_DEF,
            "fs::open(path={}, flags={:#0b})",
            path,
            flags
        );

        let session = self.do_open(selector, crt, path, flags, file_session_id)?;

        let caps = session.borrow().caps();

        // Unwrap should be okay since the do_open would otherwise return err.
        data.out_caps(caps);

        return Ok(session);
    }

    fn do_open(
        &mut self,
        srv: Selector,
        crt: usize,
        path: &str,
        flags: OpenFlags,
        file_session_id: SessId,
    ) -> Result<Rc<RefCell<FileSession>>, Error> {
        log!(
            crate::LOG_DEF,
            "fs::open(path={}, flags={:#0b}, session_idx: {})",
            path,
            flags,
            file_session_id
        );

        let ino = Dirs::search(&path, flags.contains(OpenFlags::CREATE))?;
        let inode = INodes::get(ino)?;
        let inode_mode = inode.inode().mode;

        if (flags.contains(OpenFlags::W) && !inode_mode.contains(FileMode::IWUSR))
            || (flags.contains(OpenFlags::R) && !inode_mode.contains(FileMode::IRUSR))
        {
            log!(
                crate::LOG_DEF,
                "open failed: NoPerm: opener had no permission to read or write. Flags={:b}, mode={:b}",
                flags,
                { inode.inode().mode } // {} needed because of packed inode struct
            );
            return Err(Error::new(Code::NoPerm));
        }

        // only determine the current size, if we're writing and the file isn't empty
        if flags.contains(OpenFlags::TRUNC) {
            INodes::truncate(inode.clone(), 0, 0)?;
            // TODO carried over from c++
            // TODO revoke access, if necessary
        }

        // for directories: ensure that we don't have a changed version in the cache
        if inode.inode().mode.is_dir() {
            INodes::sync_metadata(inode.clone())?;
        }
        let inode_no = inode.inode().inode;
        match self.alloc_file(srv, crt, path, flags, inode_no, file_session_id) {
            Ok(session) => {
                log!(
                    crate::LOG_DEF,
                    "-> inode={}, id={}",
                    { inode.inode().inode },
                    file_session_id
                ); // {} needed because of packed inode struct
                Ok(session)
            },
            Err(e) => Err(e),
        }
    }

    fn alloc_file(
        &mut self,
        srv: Selector,
        crt: usize,
        path: &str,
        flags: OpenFlags,
        ino: InodeNo,
        file_session_id: SessId,
    ) -> Result<Rc<RefCell<FileSession>>, Error> {
        FileSession::new(
            srv,
            crt,
            crate::REQHDL.recv_gate(),
            file_session_id,
            self.session_id,
            path,
            flags,
            ino,
        )
    }
}

impl Drop for MetaSession {
    fn drop(&mut self) {
        for g in self.sgates.iter_mut() {
            g.deactivate();
        }
    }
}

impl M3FSSession for MetaSession {
    fn creator(&self) -> usize {
        self.creator
    }

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

    fn stat(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let path: &str = stream.pop()?;

        log!(crate::LOG_DEF, "fs::stat(path={})", path);

        let ino = Dirs::search(path, false)?;
        let inode = INodes::get(ino)?;

        let mut info = FileInfo::default();
        INodes::stat(inode.clone(), &mut info);
        reply_vmsg!(stream, 0, info)
    }

    fn mkdir(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let path: &str = stream.pop()?;
        let mode = FileMode::from_bits_truncate(stream.pop::<u32>()?) & FileMode::PERM;

        log!(crate::LOG_DEF, "fs::mkdir(path={}, mode={:o})", path, mode);

        Dirs::create(path, mode)?;

        reply_vmsg!(stream, 0 as u64)
    }

    fn rmdir(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let path: &str = stream.pop()?;

        log!(crate::LOG_DEF, "fs::rmdir(path={})", path);

        Dirs::remove(path)?;

        reply_vmsg!(stream, 0 as u32)
    }

    fn link(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let old_path: &str = stream.pop()?;
        let new_path: &str = stream.pop()?;

        log!(
            crate::LOG_DEF,
            "fs::link(old_path={}, new_path: {})",
            old_path,
            new_path
        );

        Dirs::link(old_path, new_path)?;

        reply_vmsg!(stream, 0 as u32)
    }

    fn unlink(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let path: &str = stream.pop()?;
        log!(crate::LOG_DEF, "fs::unlink(path={})", path);

        Dirs::unlink(path, false)?;

        reply_vmsg!(stream, 0 as u32)
    }
}
