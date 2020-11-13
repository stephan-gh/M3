/*
 * Copyright (C) 2015-2020, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
 * Copyright (C) 2018, Sebastian Reimers <sebastian.reimers@mailbox.tu-dresden.de>
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

use crate::buf::LoadLimit;
use crate::data::{Extent, INodeRef, InodeNo};
use crate::ops::inodes;
use crate::sess::M3FSSession;

use m3::{
    cap::Selector,
    col::{String, ToString, Vec},
    com::{GateIStream, SendGate},
    errors::{Code, Error},
    kif::{CapRngDesc, CapType, Perm, INVALID_SEL},
    server::{CapExchange, SessId},
    session::ServerSession,
    syscalls, tcu,
    vfs::{FileInfo, OpenFlags, SeekMode},
};

struct Entry {
    sel: Selector,
}

impl Drop for Entry {
    fn drop(&mut self) {
        // revoke all capabilities
        m3::pes::VPE::cur()
            .revoke(
                m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, self.sel, 1),
                false,
            )
            .unwrap();
    }
}

struct CapContainer {
    caps: Vec<Entry>,
}

impl CapContainer {
    pub fn add(&mut self, sel: Selector) {
        self.caps.push(Entry { sel });
    }
}

pub struct FileSession {
    extent: usize,  // next file position: extent
    lastext: usize, // cur file position: extent

    extoff: usize,  // next file position: offset within the extent
    lastoff: usize, // cur file position: offset within the extent

    extlen: usize,  // length of current extent
    fileoff: usize, // next file position (global offset)

    lastsel: Selector,  // memory capability the client has access to currently
    lastbytes: usize,   // number of bytes the client has access to currently

    load_limit: LoadLimit,

    // for an ongoing append
    appending: bool,
    append_ext: Option<Extent>,

    // capabilities
    capscon: CapContainer,
    epcap: Selector,
    _sgate: Option<SendGate>,   // keep the send gate alive

    // the file the client has access to
    oflags: OpenFlags,
    filename: String,
    ino: InodeNo,

    // session information
    sess_sel: Selector,
    sess_creator: usize,
    session_id: SessId,
    meta_sess_id: SessId,

    _server_session: ServerSession, // keep the server session alive
}

impl FileSession {
    pub fn new(
        srv_sel: Selector,
        crt: usize,
        file_sess_id: SessId,
        meta_sess_id: SessId,
        filename: &str,
        oflags: OpenFlags,
        ino: InodeNo,
    ) -> Result<Self, Error> {
        // the server session for this file
        let sess_sel = if srv_sel == m3::kif::INVALID_SEL {
            srv_sel
        }
        else {
            m3::pes::VPE::cur().alloc_sels(2)
        };

        let _server_session =
            ServerSession::new_with_sel(srv_sel, sess_sel, crt, file_sess_id as u64, false)?;

        let send_gate = if srv_sel == m3::kif::INVALID_SEL {
            None
        }
        else {
            Some(m3::com::SendGate::new_with(
                m3::com::SGateArgs::new(crate::REQHDL.recv_gate())
                    // use the session id as identifier
                    .label(file_sess_id as tcu::Label)
                    .credits(1)
                    .sel(sess_sel + 1),
            )?)
        };

        let fsess = FileSession {
            extent: 0,
            lastext: 0,
            extoff: 0,
            lastoff: 0,
            extlen: 0,
            fileoff: 0,
            lastsel: m3::kif::INVALID_SEL,
            lastbytes: 0,
            load_limit: LoadLimit::new(),

            appending: false,
            append_ext: None,

            capscon: CapContainer { caps: vec![] },
            epcap: m3::kif::INVALID_SEL,
            _sgate: send_gate,

            oflags,
            filename: filename.to_string(),
            ino,

            sess_sel,
            sess_creator: crt,
            session_id: file_sess_id,
            meta_sess_id,

            _server_session,
        };

        crate::hdl().files().add_sess(ino);

        Ok(fsess)
    }

    pub fn clone(
        &mut self,
        srv_sel: Selector,
        crt: usize,
        sid: SessId,
        data: &mut CapExchange,
    ) -> Result<Self, Error> {
        log!(
            crate::LOG_SESSION,
            "[{}] file::clone(path={})",
            self.session_id,
            self.filename
        );

        let nsess = Self::new(
            srv_sel,
            crt,
            sid,
            self.meta_sess_id,
            &self.filename,
            self.oflags,
            self.ino,
        )?;

        data.out_caps(CapRngDesc::new(CapType::OBJECT, nsess.sess_sel, 2));

        Ok(nsess)
    }

    pub fn get_mem(&mut self, data: &mut CapExchange) -> Result<(), Error> {
        let pop_offset: u32 = data.in_args().pop()?;
        let mut offset = pop_offset as usize;

        log!(
            crate::LOG_SESSION,
            "[{}] file::get_mem(path={}, offset={})",
            self.session_id,
            self.filename,
            offset
        );

        let inode = inodes::get(self.ino)?;

        // determine extent from byte offset
        let mut first_off = offset as usize;
        let mut ext_off = 0;
        let mut tmp_extent = 0;
        inodes::seek(
            &inode,
            &mut first_off,
            SeekMode::SET,
            &mut tmp_extent,
            &mut ext_off,
        )?;
        offset = tmp_extent;

        let sel = m3::pes::VPE::cur().alloc_sel();
        let (len, _) = inodes::get_extent_mem(
            &inode,
            offset,
            ext_off,
            Perm::from(self.oflags),
            sel,
            &mut self.load_limit,
        )?;

        data.out_caps(m3::kif::CapRngDesc::new(CapType::OBJECT, sel, 1));
        data.out_args().push_word(0);
        data.out_args().push_word(len as u64);

        log!(
            crate::LOG_SESSION,
            "[{}] file::get_mem(path={}, offset={}) -> {}",
            self.session_id,
            self.filename,
            offset,
            len,
        );

        self.capscon.add(sel);

        Ok(())
    }

    pub fn set_ep(&mut self, ep: Selector) {
        self.epcap = ep;
    }

    pub fn ino(&self) -> InodeNo {
        self.ino
    }

    pub fn meta_sess(&self) -> SessId {
        self.meta_sess_id
    }

    pub fn caps(&self) -> CapRngDesc {
        CapRngDesc::new(CapType::OBJECT, self.sess_sel, 2)
    }

    fn next_in_out(&mut self, is: &mut GateIStream, out: bool) -> Result<(), Error> {
        log!(
            crate::LOG_SESSION,
            "[{}] file::next_{}(); file[path={}, fileoff={}, ext={}, extoff={}]",
            self.session_id,
            if out { "out" } else { "in" },
            self.filename,
            self.fileoff,
            self.extent,
            self.extoff
        );

        if (out && !self.oflags.contains(OpenFlags::W))
            || (!out && !self.oflags.contains(OpenFlags::R))
        {
            return Err(Error::new(Code::NoPerm));
        }

        let inode = inodes::get(self.ino)?;

        // in/out implicitly commits the previous in/out request
        if out && self.appending {
            self.commit_append(&inode, self.lastbytes)?;
        }

        let mut sel = m3::pes::VPE::cur().alloc_sel();

        // do we need to append to the file?
        let (len, extlen) = if out && (self.fileoff as u64 == inode.size) {
            let files = crate::hdl().files();
            let open_file = files.get_file_mut(self.ino).unwrap();

            if open_file.appending() {
                log!(
                    crate::LOG_SESSION,
                    "[{}] file::next_in_out(): append already in progress!",
                    self.session_id,
                );
                return Err(Error::new(Code::Exists));
            }

            // continue in last extent, if there is space
            if (self.extent > 0)
                && (self.fileoff as u64 == inode.size)
                && ((self.fileoff % crate::hdl().superblock().block_size as usize) != 0)
            {
                let mut off = 0;
                self.fileoff = inodes::seek(
                    &inode,
                    &mut off,
                    SeekMode::END,
                    &mut self.extent,
                    &mut self.extoff,
                )?;
            }

            let (len, extlen, new_ext) = inodes::req_append(
                &inode,
                self.extent,
                self.extoff,
                sel,
                Perm::from(self.oflags),
                &mut self.load_limit,
            )?;

            self.appending = true;
            self.append_ext = new_ext;

            open_file.set_appending(true);
            (len, extlen)
        }
        else {
            // get next mem_cap
            let res = inodes::get_extent_mem(
                &inode,
                self.extent,
                self.extoff,
                Perm::from(self.oflags),
                sel,
                &mut self.load_limit,
            );
            match res {
                // if we didn't find the extent, turn that into EOF
                Err(e) if e.code() == Code::NotFound => (0, 0),
                Err(e) => return Err(e),
                Ok((len, extlen)) => (len, extlen),
            }
        };

        // The mem cap covers all blocks from `self.extoff` to `self.extoff + len`. Thus, the offset
        // to start is the offset within the first of these blocks
        let mut capoff = self.extoff % crate::hdl().superblock().block_size as usize;
        if len > 0 {
            syscalls::activate(self.epcap, sel, INVALID_SEL, 0)?;

            // move forward
            self.lastoff = self.extoff;
            self.lastext = self.extent;
            if (self.extoff + len) >= extlen {
                self.extent += 1;
                self.extoff = 0;
            }
            else {
                self.extoff += len - capoff;
            }

            self.fileoff += len - capoff;
        }
        else {
            self.lastoff = 0;
            capoff = 0;
            sel = m3::kif::INVALID_SEL;
        }

        self.extlen = extlen;
        self.lastbytes = len - capoff;

        log!(
            crate::LOG_SESSION,
            "[{}] file::next_{}() -> ({}, {})",
            self.session_id,
            if out { "out" } else { "in" },
            self.lastoff,
            self.lastbytes
        );

        reply_vmsg!(is, 0 as u32, capoff, self.lastbytes)?;

        if self.lastsel != m3::kif::INVALID_SEL {
            m3::pes::VPE::cur()
                .revoke(
                    m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, self.lastsel, 1),
                    false,
                )
                .unwrap();
        }
        self.lastsel = sel;

        Ok(())
    }

    fn commit_append(&mut self, inode: &INodeRef, submit: usize) -> Result<(), Error> {
        log!(
            crate::LOG_SESSION,
            "[{}] file::commit_append(inode={}, submit={})",
            self.session_id,
            inode.inode,
            submit
        );

        assert!(submit > 0, "commit_append() submit must be > 0");

        if !self.appending {
            return Ok(());
        }

        // adjust file position
        self.fileoff -= self.lastbytes - submit;

        // add new extent?
        if let Some(ref mut append_ext) = self.append_ext.take() {
            let blocksize = crate::hdl().superblock().block_size as usize;
            let blocks = (submit + blocksize - 1) / blocksize;
            let old_len = append_ext.length;

            // append extent to file
            append_ext.length = blocks as u32;
            let new_ext = inodes::append_extent(inode, *append_ext)?;

            // free superfluous blocks
            if old_len as usize > blocks {
                crate::hdl().blocks().free(
                    append_ext.start as usize + blocks,
                    old_len as usize - blocks,
                )?;
            }

            self.extlen = blocks * blocksize;
            // have we appended the new extent to the previous extent?
            if !new_ext {
                self.extent -= 1;
            }

            self.lastoff = 0;
        }

        // we are at the end of the extent now, so move forward if not already done
        if self.extoff >= self.extlen {
            self.extent += 1;
            self.extoff = 0;
        }

        // change size
        inode.as_mut().size += submit as u64;

        // stop appending
        let files = crate::hdl().files();
        let ofile = files.get_file_mut(self.ino).unwrap();
        assert!(ofile.appending(), "ofile should be in append mode!");
        ofile.set_appending(false);

        self.append_ext = None;
        self.appending = false;

        Ok(())
    }
}

impl Drop for FileSession {
    fn drop(&mut self) {
        log!(
            crate::LOG_SESSION,
            "[{}] file::close(path={})",
            self.session_id,
            self.filename
        );

        // free to-be-appended blocks, if there are any
        if let Some(ext) = self.append_ext.take() {
            crate::hdl()
                .blocks()
                .free(ext.start as usize, ext.length as usize).unwrap();
        }

        // remove session from open_files and from its meta session
        crate::hdl().files().remove_session(self.ino).unwrap();

        // revoke caps if needed
        if self.lastsel != m3::kif::INVALID_SEL {
            m3::pes::VPE::cur().revoke(
                m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, self.lastsel, 1),
                false,
            ).unwrap();
        }
    }
}

impl M3FSSession for FileSession {
    fn creator(&self) -> usize {
        self.sess_creator
    }

    fn next_in(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        self.next_in_out(stream, false)
    }

    fn next_out(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        self.next_in_out(stream, true)
    }

    fn commit(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let nbytes: usize = stream.pop()?;

        log!(
            crate::LOG_SESSION,
            "[{}] file::commit(nbytes={}); file[path={}, fileoff={}, ext={}, extoff={}]",
            self.session_id,
            nbytes,
            self.filename,
            self.fileoff,
            self.extent,
            self.extoff
        );

        if (nbytes == 0) || (nbytes > self.lastbytes) {
            return Err(Error::new(Code::InvArgs));
        }

        let inode = inodes::get(self.ino)?;

        let res = if self.appending {
            self.commit_append(&inode, nbytes)
        }
        else {
            if (self.extent > self.lastext) && ((self.lastoff + nbytes) > self.extlen) {
                self.extent -= 1;
            }

            if nbytes < self.lastbytes {
                self.extoff = self.lastoff + nbytes;
            }
            Ok(())
        };

        self.lastbytes = 0;
        if let Err(e) = res {
            Err(e)
        }
        else {
            reply_vmsg!(stream, 0 as u32)
        }
    }

    fn seek(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let mut off: usize = stream.pop()?;
        let whence = SeekMode::from(stream.pop::<u32>()?);

        log!(
            crate::LOG_SESSION,
            "[{}] file::seek(path={}, off={}, whence={})",
            self.session_id,
            self.filename,
            off,
            whence
        );

        if whence == SeekMode::CUR {
            return Err(Error::new(Code::InvArgs));
        }

        let inode = inodes::get(self.ino)?;
        let pos = inodes::seek(&inode, &mut off, whence, &mut self.extent, &mut self.extoff)?;
        self.fileoff = pos + off;

        reply_vmsg!(stream, 0, pos, off)
    }

    fn fstat(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        log!(
            crate::LOG_SESSION,
            "[{}] file::fstat(path={})",
            self.session_id,
            self.filename
        );

        let inode = inodes::get(self.ino)?;
        let mut info = FileInfo::default();
        inode.to_file_info(&mut info);

        reply_vmsg!(stream, 0, info)
    }

    fn stat(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        self.fstat(stream)
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

    fn sync(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        log!(crate::LOG_SESSION, "[{}] file::sync()", self.session_id,);

        crate::hdl().flush_buffer()?;
        reply_vmsg!(stream, 0 as u32)
    }
}
