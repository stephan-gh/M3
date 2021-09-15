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

#![no_std]

mod addrspace;
mod dataspace;
mod mapper;
mod physmem;
mod regions;

use m3::cap::Selector;
use m3::cell::{LazyStaticCell, StaticCell};
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, MGateArgs, MemGate, RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::errors::{Code, Error, VerboseError};
use m3::format;
use m3::kif;
use m3::log;
use m3::math;
use m3::pes::{VPEArgs, VPE};
use m3::println;
use m3::server::{
    CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer, DEF_MSG_SIZE,
};
use m3::session::{ClientSession, Pager, PagerOp, ResMng, M3FS};
use m3::tcu::{Label, PEId};
use m3::vfs;

use addrspace::AddrSpace;
use resmng::childs::{self, Child, OwnChild};
use resmng::{requests, sendqueue, subsys};

pub const LOG_DEF: bool = false;

static PGHDL: LazyStaticCell<PagerReqHandler> = LazyStaticCell::default();
static REQHDL: LazyStaticCell<RequestHandler> = LazyStaticCell::default();
static MOUNTS: LazyStaticCell<Vec<(String, vfs::FSHandle)>> = LazyStaticCell::default();
static PMP_PES: StaticCell<Vec<PEId>> = StaticCell::new(Vec::new());
static SETTINGS: LazyStaticCell<PagerSettings> = LazyStaticCell::default();

struct PagerReqHandler {
    sel: Selector,
    sessions: SessionContainer<AddrSpace>,
}

impl Handler<AddrSpace> for PagerReqHandler {
    fn sessions(&mut self) -> &mut m3::server::SessionContainer<AddrSpace> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        _arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        self.sessions.add_next(crt, srv_sel, false, |sess| {
            log!(crate::LOG_DEF, "[{}] pager::open()", sess.ident());
            Ok(AddrSpace::new(crt, sess, None, None))
        })
    }

    fn obtain(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();

        let op = xchg.in_args().pop_word()? as u32;
        let sel = match PagerOp::from(op) {
            PagerOp::ADD_CHILD => {
                let sid = aspace.id();
                let child_id = aspace.child_id();
                self.sessions
                    .add_next(crt, self.sel, false, |sess| {
                        let nsid = sess.ident();
                        log!(crate::LOG_DEF, "[{}] pager::add_child(nsid={})", sid, nsid);
                        Ok(AddrSpace::new(crt, sess, Some(sid), child_id))
                    })
                    .map(|(sel, _)| sel)
            },
            PagerOp::ADD_SGATE => aspace.add_sgate(REQHDL.recv_gate()),
            _ => Err(Error::new(Code::InvArgs)),
        }?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn delegate(&mut self, _crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();

        let mut args = xchg.in_args();
        let op = args.pop_word()? as u32;
        let (sel, virt) = match PagerOp::from(op) {
            PagerOp::INIT => aspace.init(None, None).map(|sel| (sel, 0)),
            PagerOp::MAP_DS => aspace.map_ds(&mut args),
            PagerOp::MAP_MEM => aspace.map_mem(&mut args),
            _ => Err(Error::new(Code::InvArgs)),
        }?;

        if virt != 0 {
            xchg.out_args().push_word(virt);
        }

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn close(&mut self, _crt: usize, sid: SessId) {
        log!(crate::LOG_DEF, "[{}] pager::close()", sid);
        let crt = self.sessions.get(sid).unwrap().creator();
        self.sessions.remove(crt, sid);
        // ignore all potentially outstanding messages of this session
        REQHDL.recv_gate().drop_msgs_with(sid as Label);
    }
}

fn get_mount(name: &str) -> Result<vfs::FSHandle, VerboseError> {
    for (n, fs) in MOUNTS.iter() {
        if n == name {
            return Ok(fs.clone());
        }
    }

    let fs = M3FS::new(name).map_err(|e| {
        VerboseError::new(e.code(), format!("Unable to open m3fs session {}", name))
    })?;
    MOUNTS.get_mut().push((name.to_string(), fs.clone()));
    Ok(fs)
}

fn start_child_async(child: &mut OwnChild) -> Result<(), VerboseError> {
    // send gate for resmng
    #[allow(clippy::useless_conversion)]
    let resmng_sgate = SendGate::new_with(
        SGateArgs::new(requests::rgate())
            .credits(1)
            .label(Label::from(child.id())),
    )?;

    // create pager session for child (creator=0 here because we create all sessions ourself)
    let (sel, sid) = PGHDL.get_mut().open(0, PGHDL.sel, "")?;
    let sess = ClientSession::new_bind(sel);
    #[allow(clippy::useless_conversion)]
    let pager_sgate = SendGate::new_with(
        SGateArgs::new(REQHDL.recv_gate())
            .credits(1)
            .label(Label::from(sid as u32)),
    )?;

    // create child VPE
    let pe_usage = child.child_pe().unwrap();
    let mut vpe = VPE::new_with(
        pe_usage.pe_obj().clone(),
        VPEArgs::new(child.name())
            .resmng(ResMng::new(resmng_sgate))
            .pager(Pager::new(sess, pager_sgate)?)
            .kmem(child.kmem().clone()),
    )?;

    // TODO make that more flexible
    // add PMP EP for file system
    if !PMP_PES.iter().any(|id| *id == pe_usage.pe_id()) {
        let fs_mem = MemGate::new_with(MGateArgs::new(SETTINGS.fs_size, kif::Perm::R).addr(0))?;
        child
            .our_pe()
            .add_mem_region(fs_mem, SETTINGS.fs_size, true)?;
        PMP_PES.get_mut().push(pe_usage.pe_id());
    }

    // pass subsystem info to child, if it's a subsystem
    let id = child.id();
    if let Some(sub) = child.subsys() {
        sub.finalize_async(id, &mut vpe)?;
    }

    // mount file systems for childs
    for m in child.cfg().mounts() {
        let fs = get_mount(m.fs())?;
        vpe.mounts().add(m.path(), fs)?;
    }
    vpe.obtain_mounts().unwrap();

    // init address space (give it VPE and mgate selector)
    let mut aspace = PGHDL.get_mut().sessions.get_mut(sid).unwrap();
    aspace.init(Some(child.id()), Some(vpe.sel())).unwrap();

    // start VPE
    let file = vfs::VFS::open(child.name(), vfs::OpenFlags::RX)
        .map_err(|e| VerboseError::new(e.code(), format!("Unable to open {}", child.name())))?;
    let mut mapper = mapper::ChildMapper::new(&mut aspace, vpe.pe_desc().has_virtmem());
    child
        .start(vpe, &mut mapper, file)
        .map_err(|e| VerboseError::new(e.code(), "Unable to start VPE".to_string()))
}

fn handle_request(op: PagerOp, is: &mut GateIStream) -> Result<(), Error> {
    let sid = is.label() as SessId;

    // clone is special, because we need two sessions
    if op == PagerOp::CLONE {
        let pid = PGHDL.sessions.get(sid).unwrap().parent();
        if let Some(pid) = pid {
            let (sess, psess) = PGHDL.get_mut().sessions.get_two_mut(sid, pid);
            let sess = sess.unwrap();
            sess.clone(is, psess.unwrap())
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }
    else {
        let aspace = PGHDL.get_mut().sessions.get_mut(sid).unwrap();

        match op {
            PagerOp::PAGEFAULT => aspace.pagefault(is),
            PagerOp::MAP_ANON => aspace.map_anon(is),
            PagerOp::UNMAP => aspace.unmap(is),
            PagerOp::CLOSE => aspace
                .close(is)
                .map(|_| PGHDL.get_mut().close(0, is.label() as SessId)),
            _ => Err(Error::new(Code::InvArgs)),
        }
    }
}

fn workloop(serv: &Server) {
    requests::workloop(
        || {
            serv.handle_ctrl_chan(PGHDL.get_mut()).ok();

            REQHDL.get_mut().handle(handle_request).ok();
        },
        start_child_async,
    )
    .expect("Unable to run workloop");
}

#[derive(Clone, Debug)]
pub struct PagerSettings {
    fs_size: usize,
}

fn parse_args() -> Result<PagerSettings, String> {
    Ok(PagerSettings {
        fs_size: env::args()
            .last()
            .ok_or("File system size missing")?
            .parse::<usize>()
            .map_err(|_| String::from("Failed to parse FS size"))?,
    })
}

#[no_mangle]
pub fn main() -> i32 {
    SETTINGS.set(parse_args().unwrap_or_else(|e| {
        println!("Invalid arguments: {}", e);
        m3::exit(1);
    }));

    let subsys = subsys::Subsystem::new().expect("Unable to read subsystem info");

    let args = subsys.parse_args();

    // mount root FS if we haven't done that yet
    MOUNTS.set(Vec::new());
    if vfs::VFS::stat("/").is_err() {
        vfs::VFS::mount("/", "m3fs", "m3fs").expect("Unable to mount root filesystem");
    }
    MOUNTS.get_mut().push((
        "m3fs".to_string(),
        VPE::cur().mounts().get_by_path("/").unwrap(),
    ));

    // create server
    PGHDL.set(PagerReqHandler {
        sel: 0,
        sessions: SessionContainer::new(args.max_clients),
    });
    let serv = Server::new_private("pager", PGHDL.get_mut()).expect("Unable to create service");
    PGHDL.get_mut().sel = serv.sel();
    REQHDL.set(
        RequestHandler::new_with(args.max_clients, DEF_MSG_SIZE)
            .expect("Unable to create request handler"),
    );

    let mut req_rgate = RecvGate::new(
        math::next_log2(256 * args.max_clients),
        math::next_log2(256),
    )
    .expect("Unable to create resmng RecvGate");
    req_rgate
        .activate()
        .expect("Unable to activate resmng RecvGate");
    requests::init(req_rgate);

    let mut squeue_rgate = RecvGate::new(
        math::next_log2(sendqueue::RBUF_MSG_SIZE * args.max_clients),
        math::next_log2(sendqueue::RBUF_MSG_SIZE),
    )
    .expect("Unable to create sendqueue RecvGate");
    squeue_rgate
        .activate()
        .expect("Unable to activate sendqueue RecvGate");
    sendqueue::init(squeue_rgate);

    thread::init();
    // TODO calculate the number of threads we need (one per child?)
    for _ in 0..8 {
        thread::ThreadManager::get()
            .add_thread(workloop as *const () as usize, &serv as *const _ as usize);
    }

    subsys
        .start(start_child_async)
        .expect("Unable to start subsystem");

    childs::get().start_waiting(1);

    workloop(&serv);

    0
}
