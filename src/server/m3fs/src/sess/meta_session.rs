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

use crate::data::ExtPos;
use crate::ops::{dirs, inodes};
use crate::sess::{FileSession, M3FSSession};

use m3::{
    cap::Selector,
    cell::{RefCell, StaticCell},
    col::{Treap, Vec},
    com::GateIStream,
    errors::{Code, Error},
    io::LogFlags,
    kif::{CapRngDesc, CapType},
    rc::Rc,
    server::CapExchange,
    server::{ServerSession, SessId},
    vfs::{FileMode, OpenFlags},
};

static NEXT_PRIV_ID: StaticCell<SessId> = StaticCell::new(1);

pub struct FileLimit {
    max: usize,
    public: usize,
    private: usize,
}

impl FileLimit {
    pub fn new(max: usize) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            max,
            public: 0,
            private: 0,
        }))
    }

    pub fn add(&mut self, public: bool) {
        match public {
            true => self.public += 1,
            false => self.private += 1,
        }
    }

    pub fn remove(&mut self, public: bool) {
        match public {
            true => self.public -= 1,
            false => self.private -= 1,
        }
    }

    pub fn check(&self, sid: SessId) -> Result<(), Error> {
        if self.public + self.private == self.max {
            log!(
                LogFlags::Error,
                "[{}] file limit reached (priv={}, pub={})",
                sid,
                self.private,
                self.public,
            );
            Err(Error::new(Code::NoSpace))
        }
        else {
            Ok(())
        }
    }
}

pub struct MetaSession {
    serv: ServerSession,
    files: Vec<SessId>,
    priv_files: Treap<SessId, FileSession>,
    file_limit: Rc<RefCell<FileLimit>>,
    priv_eps: Vec<Selector>,
}

impl MetaSession {
    pub fn new(serv: ServerSession, file_limit: Rc<RefCell<FileLimit>>) -> Self {
        MetaSession {
            serv,
            files: Vec::new(),
            priv_files: Treap::new(),
            file_limit,
            priv_eps: Vec::new(),
        }
    }

    fn get_ep(&self, idx: usize) -> Result<Selector, Error> {
        self.priv_eps
            .get(idx)
            .copied()
            .ok_or_else(|| Error::new(Code::InvArgs))
    }

    pub fn add_ep(&mut self, ep: Selector) -> usize {
        self.priv_eps.push(ep);
        self.priv_eps.len() - 1
    }

    pub fn file_sessions(&self) -> &[SessId] {
        &self.files
    }

    pub fn remove_file(&mut self, file_session: SessId) {
        let old_count = self.files.len();
        self.files.retain(|sid| *sid != file_session);
        assert!(self.files.len() == old_count - 1);
        self.file_limit.borrow_mut().remove(true);
    }

    pub fn clone(
        &mut self,
        serv: ServerSession,
        data: &mut CapExchange<'_>,
    ) -> Result<Self, Error> {
        log!(
            LogFlags::FSSess,
            "[{}] meta::clone(nsid={})",
            self.serv.id(),
            serv.id()
        );

        // the session shares the file count with the parent to prevent that clients can sidestep
        // the limit by cloning sessions.
        let sel = serv.sel();
        let nsess = MetaSession::new(serv, self.file_limit.clone());

        data.out_caps(CapRngDesc::new(CapType::Object, sel, 2));

        Ok(nsess)
    }

    /// Creates a file session based on this meta session for `file_session_id`.
    pub fn open_file(
        &mut self,
        serv: ServerSession,
        data: &mut CapExchange<'_>,
    ) -> Result<FileSession, Error> {
        self.file_limit.borrow().check(self.serv.id())?;

        let args = data.in_args();
        let flags: OpenFlags = args.pop()?;
        let path: &str = args.pop()?;

        log!(
            LogFlags::FSSess,
            "[{}] meta::open(path={}, flags={:?})",
            self.serv.id(),
            path,
            flags
        );

        let sid = serv.id();
        let sel = serv.sel();
        let session = self.do_open(Some(serv), sid, path, flags)?;

        self.files.push(sid);
        self.file_limit.borrow_mut().add(true);

        data.out_caps(CapRngDesc::new(CapType::Object, sel, 2));

        log!(
            LogFlags::FSSess,
            "[{}] meta::open(path={}, flags={:?}) -> inode={}, sid={}",
            self.serv.id(),
            path,
            flags,
            session.ino(),
            sid,
        );

        Ok(session)
    }

    fn do_open(
        &mut self,
        serv: Option<ServerSession>,
        id: SessId,
        path: &str,
        flags: OpenFlags,
    ) -> Result<FileSession, Error> {
        self.file_limit.borrow().check(self.serv.id())?;

        let ino = dirs::search(path, flags.contains(OpenFlags::CREATE))?;
        let inode = inodes::get(ino)?;
        let inode_mode = inode.mode;

        if (flags.contains(OpenFlags::W) && !inode_mode.contains(FileMode::IWUSR))
            || (flags.contains(OpenFlags::R) && !inode_mode.contains(FileMode::IRUSR))
        {
            log!(
                LogFlags::FSSess,
                "insufficient permissions: flags={:o}, mode={:o}",
                flags,
                inode.mode,
            );
            return Err(Error::new(Code::NoPerm));
        }

        // only determine the current size, if we're writing and the file isn't empty
        if flags.contains(OpenFlags::TRUNC) {
            inodes::truncate(&inode, &ExtPos::new(0, 0))?;
            // TODO revoke access, if necessary
        }

        // for directories: ensure that we don't have a changed version in the cache
        if inode.mode.is_dir() {
            inodes::sync_metadata(&inode)?;
        }

        FileSession::new(
            serv,
            None,
            id,
            self.serv.id(),
            self.file_limit.clone(),
            path,
            flags,
            inode.inode,
        )
    }

    fn with_file_sess<F>(&mut self, stream: &mut GateIStream<'_>, func: F) -> Result<(), Error>
    where
        F: Fn(&mut FileSession, &mut GateIStream<'_>) -> Result<(), Error>,
    {
        let fid: usize = stream.pop()?;
        match self.priv_files.get_mut(&fid) {
            Some(f) => func(f, stream),
            None => Err(Error::new(Code::InvArgs)),
        }
    }
}

impl M3FSSession for MetaSession {
    fn next_in(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        self.with_file_sess(stream, |f, stream| f.file_in_out(stream, false))
    }

    fn next_out(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        self.with_file_sess(stream, |f, stream| f.file_in_out(stream, true))
    }

    fn commit(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        self.with_file_sess(stream, |f, stream| f.file_commit(stream))
    }

    fn seek(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        self.with_file_sess(stream, |f, stream| f.file_seek(stream))
    }

    fn stat(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        self.with_file_sess(stream, |f, stream| f.file_stat(stream))
    }

    fn get_path(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        self.with_file_sess(stream, |f, stream| f.file_path(stream))
    }

    fn truncate(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        self.with_file_sess(stream, |f, stream| f.file_truncate(stream))
    }

    fn sync(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        self.with_file_sess(stream, |f, stream| f.file_sync(stream))
    }

    fn fstat(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        let path: &str = stream.pop()?;

        log!(
            LogFlags::FSSess,
            "[{}] meta::stat(path={})",
            self.serv.id(),
            path
        );

        let ino = dirs::search(path, false)?;
        let inode = inodes::get(ino)?;

        let info = inode.to_file_info();

        let mut reply = m3::mem::MsgBuf::borrow_def();
        build_vmsg!(reply, Code::Success, info);
        stream.reply(&reply)
    }

    fn mkdir(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        let path: &str = stream.pop()?;
        let mode = FileMode::from_bits_truncate(stream.pop::<u16>()?) & FileMode::PERM;

        log!(
            LogFlags::FSSess,
            "[{}] meta::mkdir(path={}, mode={:o})",
            self.serv.id(),
            path,
            mode
        );

        dirs::create(path, mode)?;

        stream.reply_error(Code::Success)
    }

    fn rmdir(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        let path: &str = stream.pop()?;

        log!(
            LogFlags::FSSess,
            "[{}] meta::rmdir(path={})",
            self.serv.id(),
            path
        );

        dirs::remove(path)?;

        stream.reply_error(Code::Success)
    }

    fn link(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        let old_path: &str = stream.pop()?;
        let new_path: &str = stream.pop()?;

        log!(
            LogFlags::FSSess,
            "[{}] meta::link(old_path={}, new_path: {})",
            self.serv.id(),
            old_path,
            new_path
        );

        dirs::link(old_path, new_path)?;

        stream.reply_error(Code::Success)
    }

    fn unlink(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        let path: &str = stream.pop()?;

        log!(
            LogFlags::FSSess,
            "[{}] meta::unlink(path={})",
            self.serv.id(),
            path
        );

        dirs::unlink(path, true)?;

        stream.reply_error(Code::Success)
    }

    fn rename(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        let old_path: &str = stream.pop()?;
        let new_path: &str = stream.pop()?;

        log!(
            LogFlags::FSSess,
            "[{}] meta::rename(old_path={}, new_path: {})",
            self.serv.id(),
            old_path,
            new_path
        );

        dirs::rename(old_path, new_path)?;

        stream.reply_error(Code::Success)
    }

    fn open_priv(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        let path = stream.pop::<&str>()?;
        let flags = OpenFlags::from_bits_truncate(stream.pop::<u32>()?);
        let ep = stream.pop::<usize>()?;

        log!(
            LogFlags::FSSess,
            "[{}] meta::open_priv(path={}, flags={:?}, ep={})",
            self.serv.id(),
            path,
            flags,
            ep
        );

        let ep_sel = self.get_ep(ep)?;

        let id = NEXT_PRIV_ID.get();
        let mut session = self.do_open(None, id, path, flags)?;
        session.set_ep(ep_sel);
        NEXT_PRIV_ID.set(id + 1);

        log!(
            LogFlags::FSSess,
            "[{}] meta::open_priv(path={}, flags={:?}) -> inode={}, sid={}",
            self.serv.id(),
            path,
            flags,
            session.ino(),
            id,
        );

        self.priv_files.insert(id, session);
        self.file_limit.borrow_mut().add(false);

        reply_vmsg!(stream, 0, id)
    }

    fn close_priv(&mut self, stream: &mut GateIStream<'_>) -> Result<(), Error> {
        let fid = stream.pop::<SessId>()?;

        if self.priv_files.remove(&fid).is_some() {
            self.file_limit.borrow_mut().remove(false);
            stream.reply_error(Code::Success)
        }
        else {
            stream.reply_error(Code::InvArgs)
        }
    }
}
