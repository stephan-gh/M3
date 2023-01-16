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

#![no_std]

mod addrspace;
mod dataspace;
mod mapper;
mod physmem;
mod regions;

use core::ops::DerefMut;

use m3::boxed::Box;
use m3::cap::Selector;
use m3::cell::{LazyReadOnlyCell, LazyStaticRefCell};
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, MemGate, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error, VerboseError};
use m3::format;
use m3::kif;
use m3::log;
use m3::server::{CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer};
use m3::session::{ClientSession, Pager, PagerOp, ResMng, M3FS};
use m3::tcu::Label;
use m3::tiles::{Activity, ActivityArgs, ChildActivity};
use m3::util::math;
use m3::vfs;

use addrspace::AddrSpace;

use resmng::childs::{self, Child, ChildManager, OwnChild};
use resmng::config;
use resmng::requests;
use resmng::resources::{tiles, Resources};
use resmng::sendqueue;
use resmng::subsys;

pub const LOG_DEF: bool = false;

static PGHDL: LazyStaticRefCell<PagerReqHandler> = LazyStaticRefCell::default();
static REQHDL: LazyReadOnlyCell<RequestHandler> = LazyReadOnlyCell::default();
static MOUNTS: LazyStaticRefCell<Vec<(String, String)>> = LazyStaticRefCell::default();

struct PagerReqHandler {
    sel: Selector,
    sessions: SessionContainer<AddrSpace>,
}

impl PagerReqHandler {
    fn close_sess(&mut self, _crt: usize, sid: SessId, rgate: &RecvGate) {
        log!(crate::LOG_DEF, "[{}] pager::close()", sid);
        let crt = self.sessions.get(sid).unwrap().creator();
        self.sessions.remove(crt, sid);
        // ignore all potentially outstanding messages of this session
        rgate.drop_msgs_with(sid as Label).unwrap();
    }
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

    fn obtain(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange<'_>) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();

        let args = xchg.in_args();
        let sel = match args.pop()? {
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
            PagerOp::ADD_SGATE => aspace.add_sgate(REQHDL.get().recv_gate()),
            _ => Err(Error::new(Code::InvArgs)),
        }?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn delegate(
        &mut self,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let aspace = self.sessions.get_mut(sid).unwrap();

        let args = xchg.in_args();
        let (sel, virt) = match args.pop()? {
            PagerOp::INIT => aspace.init(None, None).map(|sel| (sel, 0)),
            PagerOp::MAP_DS => aspace.map_ds(args),
            PagerOp::MAP_MEM => aspace.map_mem(args),
            _ => Err(Error::new(Code::InvArgs)),
        }?;

        if virt != 0 {
            xchg.out_args().push(virt);
        }

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn close(&mut self, _crt: usize, sid: SessId) {
        self.close_sess(_crt, sid, REQHDL.get().recv_gate());
    }
}

fn get_mount(name: &str) -> Result<String, VerboseError> {
    for (n, mpath) in MOUNTS.borrow().iter() {
        if n == name {
            return Ok(mpath.clone());
        }
    }

    let id = MOUNTS.borrow().len();
    let fs = M3FS::new(id, name).map_err(|e| {
        VerboseError::new(e.code(), format!("Unable to open m3fs session {}", name))
    })?;
    let our_path = format!("/child-mount-{}", name);
    Activity::own().mounts().add(&our_path, fs)?;
    MOUNTS
        .borrow_mut()
        .push((name.to_string(), our_path.to_string()));
    Ok(our_path)
}

struct PagedChildStarter {}

impl subsys::ChildStarter for PagedChildStarter {
    fn start(
        &mut self,
        reqs: &requests::Requests,
        res: &mut Resources,
        child: &mut OwnChild,
    ) -> Result<(), VerboseError> {
        // send gate for resmng
        let resmng_sgate = SendGate::new_with(
            SGateArgs::new(reqs.recv_gate())
                .credits(1)
                .label(Label::from(child.id())),
        )?;

        // create pager session for child (creator=0 here because we create all sessions ourself)
        let (sel, sid, child_sgate) = {
            let mut hdl = PGHDL.borrow_mut();
            let srv_sel = hdl.sel;
            let (sel, sid) = hdl.open(0, srv_sel, "")?;
            let aspace = hdl.sessions.get_mut(sid).unwrap();
            let child_sgate = aspace.add_sgate(REQHDL.get().recv_gate()).unwrap();
            (sel, sid, child_sgate)
        };
        let sess = ClientSession::new_bind(sel);
        let pager_sgate = SendGate::new_with(
            SGateArgs::new(REQHDL.get().recv_gate())
                .credits(1)
                .label(Label::from(sid as u32)),
        )?;

        // create child activity
        let mut act = ChildActivity::new_with(
            child.child_tile().unwrap().tile_obj().clone(),
            ActivityArgs::new(child.name())
                .resmng(ResMng::new(resmng_sgate))
                .pager(Pager::new(sess, pager_sgate, child_sgate)?)
                .kmem(child.kmem().unwrap()),
        )?;

        // pass subsystem info to child, if it's a subsystem
        let id = child.id();
        if let Some(sub) = child.subsys() {
            sub.finalize_async(res, id, &mut act)?;
        }

        // mount file systems for childs
        for m in child.cfg().mounts() {
            let path = get_mount(m.fs())?;
            act.add_mount(m.path(), &path);
        }

        // init address space (give it activity and mgate selector)
        let mut hdl = PGHDL.borrow_mut();
        let aspace = hdl.sessions.get_mut(sid).unwrap();
        aspace.init(Some(child.id()), Some(act.sel())).unwrap();

        // start activity
        let file = vfs::VFS::open(child.name(), vfs::OpenFlags::RX | vfs::OpenFlags::NEW_SESS)
            .map_err(|e| VerboseError::new(e.code(), format!("Unable to open {}", child.name())))?;
        let mut mapper = mapper::ChildMapper::new(aspace, act.tile_desc().has_virtmem());

        let run = act
            .exec_file(&mut mapper, file.into_generic(), child.arguments())
            .map_err(|e| {
                VerboseError::new(e.code(), format!("Unable to execute {}", child.name()))
            })?;

        child.set_running(Box::new(run));

        Ok(())
    }

    fn configure_tile(
        &mut self,
        _res: &mut Resources,
        tile: &tiles::TileUsage,
        _domain: &config::Domain,
    ) -> Result<(), VerboseError> {
        let fs_mod = MemGate::new_bind_bootmod("fs")?;
        let fs_mod_size = fs_mod.region()?.1 as usize;
        // don't overwrite PMP EPs here, but use the next free one. this is required in case we
        // share our tile with this child and therefore need to add a PMP EP for ourself. Since our
        // parent has already set PMP EPs, we don't want to overwrite them.
        tile.add_mem_region(fs_mod, fs_mod_size, true, false)
            .map_err(|e| {
                VerboseError::new(e.code(), "Unable to add PMP EP for FS image".to_string())
            })
    }
}

fn handle_request(
    childs: &mut ChildManager,
    op: PagerOp,
    is: &mut GateIStream<'_>,
) -> Result<(), Error> {
    let mut hdl = PGHDL.borrow_mut();
    let sid = is.label() as SessId;

    // clone is special, because we need two sessions
    if op == PagerOp::CLONE {
        let pid = hdl.sessions.get(sid).unwrap().parent();
        if let Some(pid) = pid {
            let (sess, psess) = hdl.sessions.get_two_mut(sid, pid);
            let sess = sess.unwrap();
            sess.clone(is, psess.unwrap())
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }
    else {
        let aspace = hdl.sessions.get_mut(sid).unwrap();

        match op {
            PagerOp::PAGEFAULT => aspace.pagefault(childs, is),
            PagerOp::MAP_ANON => aspace.map_anon(is),
            PagerOp::UNMAP => aspace.unmap(is),
            PagerOp::CLOSE => aspace
                .close(is)
                .map(|_| hdl.close_sess(0, is.label() as SessId, is.rgate())),
            _ => Err(Error::new(Code::InvArgs)),
        }
    }
}

#[allow(clippy::vec_box)]
struct WorkloopArgs<'c, 'd, 'r, 'q, 's> {
    childs: &'c mut ChildManager,
    delayed: &'d mut Vec<Box<OwnChild>>,
    res: &'r mut Resources,
    reqs: &'q requests::Requests,
    serv: &'s mut Server,
}

fn workloop(args: &mut WorkloopArgs<'_, '_, '_, '_, '_>) {
    let WorkloopArgs {
        childs,
        delayed,
        res,
        reqs,
        serv,
    } = args;

    reqs.run_loop(
        childs,
        delayed,
        res,
        |childs, _res| {
            serv.handle_ctrl_chan(PGHDL.borrow_mut().deref_mut()).ok();

            REQHDL
                .get()
                .handle(|op, is| handle_request(childs, op, is))
                .ok();
        },
        &mut PagedChildStarter {},
    )
    .expect("Unable to run workloop");
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let (subsys, mut res) = subsys::Subsystem::new().expect("Unable to read subsystem info");

    let args = subsys.parse_args();
    for sem in &args.sems {
        res.semaphores_mut()
            .add_sem(sem.clone())
            .expect("Unable to add semaphore");
    }

    // mount root FS if we haven't done that yet
    MOUNTS.set(Vec::new());
    if vfs::VFS::stat("/").is_err() {
        vfs::VFS::mount("/", "m3fs", "m3fs").expect("Unable to mount root filesystem");
    }
    MOUNTS
        .borrow_mut()
        .push(("m3fs".to_string(), "/".to_string()));

    // create server
    let mut hdl = PagerReqHandler {
        sel: 0,
        sessions: SessionContainer::new(args.max_clients),
    };
    let mut serv = Server::new_private("pager", &mut hdl).expect("Unable to create service");
    hdl.sel = serv.sel();
    PGHDL.set(hdl);

    REQHDL.set(
        RequestHandler::new_with(args.max_clients, 128).expect("Unable to create request handler"),
    );

    let req_rgate = RecvGate::new(
        math::next_log2(256 * args.max_clients),
        math::next_log2(256),
    )
    .expect("Unable to create resmng RecvGate");
    // manually activate the RecvGate here, because it requires quite a lot of EPs and we are
    // potentially moving (<EPs left> - 16) EPs to a child activity. therefore, we should allocate
    // all EPs before starting childs.
    req_rgate
        .activate()
        .expect("Unable to activate resmng RecvGate");
    let reqs = requests::Requests::new(req_rgate);

    let squeue_rgate = RecvGate::new(
        math::next_log2(sendqueue::RBUF_MSG_SIZE * args.max_clients),
        math::next_log2(sendqueue::RBUF_MSG_SIZE),
    )
    .expect("Unable to create sendqueue RecvGate");
    squeue_rgate
        .activate()
        .expect("Unable to activate sendqueue RecvGate");
    sendqueue::init(squeue_rgate);

    let mut childs = childs::ChildManager::default();

    let mut delayed = subsys
        .start(&mut childs, &reqs, &mut res, &mut PagedChildStarter {})
        .expect("Unable to start subsystem");

    let mut wargs = WorkloopArgs {
        childs: &mut childs,
        delayed: &mut delayed,
        res: &mut res,
        reqs: &reqs,
        serv: &mut serv,
    };

    thread::init();
    for _ in 0..args.max_clients {
        thread::add_thread(
            workloop as *const () as usize,
            &mut wargs as *mut _ as usize,
        );
    }

    wargs.childs.start_waiting(1);

    workloop(&mut wargs);

    Ok(())
}
