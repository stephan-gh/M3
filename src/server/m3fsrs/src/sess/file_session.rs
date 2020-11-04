use crate::data::*;
use crate::internal::*;
use crate::sess::{M3FSSession, Request};
use crate::util::*;

use m3::{
    cap::Selector,
    cell::RefCell,
    col::{String, Vec},
    com::{GateIStream, SendGate},
    errors::{Code, Error},
    kif::{CapRngDesc, CapType, INVALID_SEL},
    rc::Rc,
    serialize::Sink,
    server::{CapExchange, SessId},
    session::ServerSession,
    syscalls, tcu,
};

struct Entry {
    sel: Selector,
}

impl Drop for Entry {
    fn drop(&mut self) {
        //On drop, revoke all capabilities
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
    extent: usize,
    lastext: usize,

    extoff: usize,
    lastoff: usize,

    extlen: usize,
    fileoff: usize,

    lastbytes: usize,

    accessed: usize,

    appending: bool,
    pub(crate) append_ext: Option<LoadedExtent>,

    pub(crate) last: Selector,
    epcap: Selector,
    #[allow(dead_code)] //keeps the send gate alive
    sgate: Option<SendGate>,

    oflags: u64,
    filename: String,
    ino: InodeNo,

    ///the selector this session was created for
    sel: Selector,
    creator: usize,
    ///The id of the parent meta session
    pub(crate) meta_session: SessId,

    capscon: CapContainer,
    #[allow(dead_code)] //keeps the server session alive
    server_session: ServerSession,
}

impl Drop for FileSession {
    fn drop(&mut self) {
        log!(crate::LOG_DEF, "file:close(path={})", self.filename);
    }
}

impl FileSession {
    pub fn new(
        srv_sel: Selector,
        crt: usize,
        meta_rgate: &m3::com::RecvGate,
        file_session_id: SessId,
        meta_session_id: SessId,
        filename: String,
        flags: u64,
        ino: InodeNo,
    ) -> Result<Rc<RefCell<Self>>, Error> {
        log!(
            crate::LOG_DEF,
            "Creating File Session (filename={}, inode={}, file_session_id={})",
            filename,
            ino,
            file_session_id
        );

        //The server session for this file
        let sel = if srv_sel == m3::kif::INVALID_SEL {
            srv_sel
        }
        else {
            m3::pes::VPE::cur().alloc_sels(2)
        };

        let server_session =
            ServerSession::new_with_sel(srv_sel, sel, crt, file_session_id as u64, false)?;

        let send_gate = if srv_sel == m3::kif::INVALID_SEL {
            None
        }
        else {
            Some(m3::com::SendGate::new_with(
                m3::com::SGateArgs::new(meta_rgate)
                    //We use the file session id as identifier when the session is called again.
                    //The olf impl used the pointer to this session, but this is not as easy in rust and I guess
                    // kinda unsafe as well
                    .label(file_session_id as tcu::Label)
                    .credits(1)
                    .sel(sel + 1),
            )?)
        };

        let fsess = FileSession {
            extent: 0,
            lastext: 0,
            extoff: 0,
            lastoff: 0,
            extlen: 0,
            fileoff: 0,
            lastbytes: 0,
            accessed: 0,

            appending: false,
            append_ext: None,

            last: m3::kif::INVALID_SEL,
            epcap: m3::kif::INVALID_SEL,
            sgate: send_gate,

            oflags: flags,
            filename,
            ino,

            sel,
            creator: crt,
            meta_session: meta_session_id,

            capscon: CapContainer { caps: vec![] },

            server_session,
        };

        let wrapped_fssess = Rc::new(RefCell::new(fsess));

        crate::hdl().files().add_sess(wrapped_fssess.clone());

        Ok(wrapped_fssess)
    }

    pub fn clone(&mut self, _selector: Selector, _data: &mut CapExchange) -> Result<(), Error> {
        log!(crate::LOG_DEF, "file:clone(path={})", self.filename);

        panic!("Clone not yet implemented")
    }

    pub fn get_mem(&mut self, data: &mut CapExchange) -> Result<(), Error> {
        let pop_offset: u32 = data.in_args().pop().expect("Failed to pop mem offset");
        let mut offset = pop_offset as usize;
        let mut req = Request::new();

        log!(
            crate::LOG_DEF,
            "file::get_mem(path={}, offset={})",
            self.filename,
            offset
        );

        let inode = INodes::get(&mut req, self.ino);

        let mut first_off = offset as usize;
        let mut ext_off = 0;
        let mut tmp_extent = 0;
        let _some = INodes::seek(
            &mut req,
            inode.clone(),
            &mut first_off,
            M3FS_SEEK_SET,
            &mut tmp_extent,
            &mut ext_off,
        );
        offset = tmp_extent;
        let sel = m3::pes::VPE::cur().alloc_sel();

        let mut extlen = 0;
        let len = INodes::get_extent_mem(
            &mut req,
            inode.clone(),
            offset,
            ext_off,
            &mut extlen,
            flags_to_perm(self.oflags),
            sel,
            true,
            self.accessed,
        );

        if req.has_error() {
            log!(
                crate::LOG_DEF,
                "getting extent memory failed: {:?}",
                req.error().unwrap()
            );
            return Err(Error::new(req.error().unwrap()));
        }

        data.out_caps(m3::kif::CapRngDesc::new(CapType::OBJECT, sel, 1));
        data.out_args().push(&0);
        data.out_args().push(&len);

        log!(crate::LOG_DEF, "file::get_mem -> {}", len);
        self.capscon.add(sel);
        return Ok(());
    }

    pub fn set_ep(&mut self, ep: Selector) {
        self.epcap = ep;
    }

    pub fn ino(&self) -> InodeNo {
        self.ino
    }

    pub fn caps(&self) -> CapRngDesc {
        CapRngDesc::new(CapType::OBJECT, self.sel, 2)
    }

    fn next_in_out(&mut self, is: &mut GateIStream, out: bool) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "file::next_{}(); file[path={}, fileoff={}, ext={}, extoff={}]",
            if out { "out" } else { "in" },
            self.filename,
            self.fileoff,
            self.extent,
            self.extoff
        );

        if (out && ((self.oflags & FILE_W) == 0)) || (!out && ((self.oflags & FILE_R) == 0)) {
            return Err(Error::new(Code::NoPerm));
        }

        let mut req = Request::new();
        let inode = INodes::get(&mut req, self.ino);
        // in/out implicitly commits the previous in/out request
        if out && self.appending {
            if let Err(e) = self.commit_append(&mut req, inode.clone(), self.lastbytes) {
                return Err(e);
            }
        }

        if self.accessed < 31 {
            self.accessed += 1;
        }

        let mut sel = m3::pes::VPE::cur().alloc_sel();
        let mut extlen = 0;

        //Do we need to append to the file?
        let len = if out && (self.fileoff as u64 == inode.inode().size) {
            let mut files = crate::hdl().files();
            let open_file = if let Some(f) = files.get_file_mut(self.ino) {
                f
            }
            else {
                panic!("Could not get open file for next_in_out operation in file session");
            };

            if open_file.appending() {
                log!(
                    crate::LOG_DEF,
                    "file::next_in_out : append already in progress!"
                );
                return Err(Error::new(Code::Exists));
            }

            //Continue in last extent if there is space
            if (self.extent > 0)
                && (self.fileoff as u64 == inode.inode().size)
                && ((self.fileoff % crate::hdl().superblock().block_size as usize) != 0)
            {
                let mut off = 0;
                self.fileoff = INodes::seek(
                    &mut req,
                    inode.clone(),
                    &mut off,
                    M3FS_SEEK_END,
                    &mut self.extent,
                    &mut self.extoff,
                );
            }
            //Exchange extent in which we store the "to append" extent
            let mut e = LoadedExtent::Unstored {
                extent: Rc::new(RefCell::new(Extent {
                    start: 0,
                    length: 0,
                })),
            };

            let len = INodes::req_append(
                &mut req,
                inode.clone(),
                self.extent,
                self.extoff,
                &mut extlen,
                sel,
                flags_to_perm(self.oflags),
                &mut e,
                self.accessed,
            );

            if req.has_error() {
                log!(crate::LOG_DEF, "append failed: {:?}", req.error().unwrap());
                return Err(Error::new(req.error().unwrap()));
            }

            self.appending = true;
            self.append_ext = if *e.length() > 0 { Some(e) } else { None };

            open_file.set_appending(true);
            len
        }
        else {
            //get next mem_cap
            let len = INodes::get_extent_mem(
                &mut req,
                inode.clone(),
                self.extent,
                self.extoff,
                &mut extlen,
                flags_to_perm(self.oflags),
                sel,
                out,
                self.accessed,
            );
            if req.has_error() {
                log!(
                    crate::LOG_DEF,
                    "getting extent memory failed: {:?}",
                    req.error().unwrap()
                );
                return Err(Error::new(req.error().unwrap()));
            }
            len
        };

        //The mem cap covers all blocks from `self.extoff` to `self.extoff + len`. Thus, the offset to start
        // is the offset within the first of these blocks
        let mut capoff = self.extoff % crate::hdl().superblock().block_size as usize;
        if len > 0 {
            if let Err(e) = syscalls::activate(self.epcap, sel, INVALID_SEL, 0) {
                log!(crate::LOG_DEF, "activate failed: {:?}", e.code());
                //is.reply_error(e.code());
                return Err(e);
            }

            //Move forward
            self.lastoff = self.extoff;
            self.lastext = self.extent;
            if (self.extoff + len) >= extlen {
                self.extent += 1;
                self.extoff = 0;
            }
            else {
                self.extoff += len - self.extoff % crate::hdl().superblock().block_size as usize;
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
            crate::LOG_DEF,
            "file::next_{}() -> ({}, {})",
            if out { "out" } else { "in" },
            self.lastoff,
            self.lastbytes
        );

        if crate::hdl().revoke_first() {
            //revoke last mem cap and remember new one
            if self.last != m3::kif::INVALID_SEL {
                m3::pes::VPE::cur()
                    .revoke(
                        m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, self.last, 1),
                        false,
                    )
                    .unwrap();
            }
            self.last = sel;
            reply_vmsg!(is, 0 as u32, capoff, self.lastbytes)
        }
        else {
            reply_vmsg!(is, 0 as u32, capoff, self.lastbytes).unwrap();
            if self.last != m3::kif::INVALID_SEL {
                m3::pes::VPE::cur()
                    .revoke(
                        m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, self.last, 1),
                        false,
                    )
                    .unwrap();
            }
            self.last = sel;
            Ok(())
        }
    }

    fn commit_append(
        &mut self,
        req: &mut Request,
        inode: LoadedInode,
        submit: usize,
    ) -> Result<(), Error> {
        assert!(submit > 0, "commit_append() submit must be > 0");
        log!(
            crate::LOG_DEF,
            "file::commit_append(inode={}, submit={})",
            { inode.inode().inode },
            submit
        );
        if !self.appending {
            return Ok(());
        }

        //adjust file position
        self.fileoff -= self.lastbytes - submit;

        //add new extent?
        if let Some(ref append_ext) = self.append_ext {
            let blocksize = crate::hdl().superblock().block_size as usize;
            let blocks = (submit + blocksize - 1) / blocksize;
            let old_len = *append_ext.length();
            //append extent to file
            *append_ext.length_mut() = blocks as u32;
            let mut new_ext = false;
            if let Err(e) = INodes::append_extent(req, inode.clone(), &append_ext, &mut new_ext) {
                return Err(e);
            }

            //free superfluous blocks
            if old_len as usize > blocks {
                crate::hdl().blocks().free(
                    req,
                    *append_ext.start() as usize + blocks,
                    old_len as usize - blocks,
                );
            }

            self.extlen = blocks * blocksize;
            //have we appended the new extent to the previous extent?
            if !new_ext {
                self.extent -= 1;
            }

            self.lastoff = 0;
            self.append_ext = None;
        }

        // we are at the end of the extent now, so move forward if not already done
        if self.extoff >= self.extlen {
            self.extent += 1;
            self.extoff = 0;
        }

        //change size
        inode.inode().size += submit as u64;
        INodes::mark_dirty(req, inode.inode().inode);

        //stop appending
        let mut files = crate::hdl().files();
        let ofile = if let Some(f) = files.get_file_mut(self.ino) {
            f
        }
        else {
            panic!("Could not get file for file session while commiting append");
        };

        assert!(ofile.appending(), "ofile should be in append mode!");
        ofile.set_appending(false);

        self.append_ext = None;
        self.appending = false;

        Ok(())
    }

    #[allow(dead_code)] //TODO currently unused since there seams to be no SYNC Op in rust
    fn sync(&mut self, stream: &mut GateIStream) {
        crate::hdl().flush_buffer();
        reply_vmsg!(stream, 0 as u32).unwrap();
    }
}

impl M3FSSession for FileSession {
    fn creator(&self) -> usize {
        self.creator
    }

    fn next_in(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        self.next_in_out(stream, false)
    }

    fn next_out(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        self.next_in_out(stream, true)
    }

    fn commit(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        //TODO is it correct to pop usize here? size_t can be everything >= 16bit in c++ so I'm

        // not really sure what to use here.
        let nbytes: usize = if let Ok(nb) = stream.pop() {
            nb
        }
        else {
            log!(crate::LOG_DEF, "Could not get commit parameter");
            //stream.reply_error(Code::InvArgs);
            return Err(Error::new(Code::InvArgs));
        };

        let mut req = Request::new();

        log!(
            crate::LOG_DEF,
            "file::commit(nbytes={}); file[path={}, fileoff={}, ext={}, extoff={}]",
            nbytes,
            self.filename,
            self.fileoff,
            self.extent,
            self.extoff
        );

        if (nbytes == 0) || (nbytes > self.lastbytes) {
            //stream.reply_error(Code::InvArgs);
            return Err(Error::new(Code::InvArgs));
        }

        let inode = INodes::get(&mut req, self.ino);

        let res = if self.appending {
            self.commit_append(&mut req, inode.clone(), nbytes)
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
            //stream.reply_error(e.code());
            Err(e)
        }
        else {
            reply_vmsg!(stream, 0 as u32)
        }
    }

    fn seek(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let (whence, mut off): (i32, usize) = if let (Ok(o), Ok(w)) = (stream.pop(), stream.pop()) {
            (w, o)
        }
        else {
            log!(
                crate::LOG_DEF,
                "Could not get whnce of offset while seeking in file_session"
            );
            //stream.reply_error(Code::InvArgs);
            return Err(Error::new(Code::InvArgs));
        };

        log!(
            crate::LOG_DEF,
            "file::seek(path={}, off={}, whence={})",
            self.filename,
            off,
            whence
        );
        if whence == M3FS_SEEK_CUR {
            //stream.reply_error(Code::InvArgs);
            return Err(Error::new(Code::InvArgs));
        }

        let mut req = Request::new();
        let inode = INodes::get(&mut req, self.ino);

        let pos = INodes::seek(
            &mut req,
            inode.clone(),
            &mut off,
            whence,
            &mut self.extent,
            &mut self.extoff,
        );
        self.fileoff = pos + off;
        reply_vmsg!(stream, 0, pos, off)
    }

    fn fstat(&mut self, stream: &mut GateIStream) -> Result<(), Error> {
        let mut req = Request::new();
        log!(crate::LOG_DEF, "file::fstat(path={})", self.filename);
        let inode = INodes::get(&mut req, self.ino);

        let mut info = FileInfo::default();
        INodes::stat(&mut req, inode.clone(), &mut info);

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
}
