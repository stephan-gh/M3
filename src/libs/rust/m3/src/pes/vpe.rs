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

//! Contains the VPE abstraction

use base::envdata;

use core::cmp;
use core::fmt;

use crate::arch;
use crate::cap::{CapFlags, Capability, Selector};
use crate::cell::LazyStaticUnsafeCell;
use crate::col::Vec;
use crate::com::{EpMng, MemGate};
use crate::env;
use crate::errors::Error;
use crate::goff;
use crate::kif;
use crate::kif::{CapRngDesc, CapType, PEDesc};
use crate::pes::{
    DefaultMapper, DeviceActivity, ExecActivity, KMem, Mapper, StateDeserializer, StateSerializer,
    PE,
};
use crate::pexif;
use crate::rc::Rc;
use crate::session::{Pager, ResMng};
use crate::syscalls;
use crate::tcu::{EpId, PEId, VPEId, INVALID_EP, TCU};
use crate::time::TimeDuration;
use crate::vfs::{BufReader, FileRef, OpenFlags, VFS};
use crate::vfs::{FileTable, MountTable};

/// A virtual processing element is used to run an activity on a PE.
pub struct VPE {
    id: VPEId,
    cap: Capability,
    rmng: Option<ResMng>, // close the connection resource manager at last
    pe: Rc<PE>,
    kmem: Rc<KMem>,
    next_sel: Selector,
    #[allow(dead_code)]
    eps_start: EpId,
    epmng: EpMng,
    pager: Option<Pager>,
    files: FileTable,
    mounts: MountTable,
    data: Vec<u64>,
}

/// The arguments for [`VPE`] creations.
pub struct VPEArgs<'n> {
    name: &'n str,
    pager: Option<Pager>,
    kmem: Option<Rc<KMem>>,
    rmng: Option<ResMng>,
}

impl<'n> VPEArgs<'n> {
    /// Creates a new instance of `VPEArgs` using default settings.
    pub fn new(name: &'n str) -> VPEArgs<'n> {
        VPEArgs {
            name,
            pager: None,
            kmem: None,
            rmng: None,
        }
    }

    /// Sets the resource manager to `rmng`. Otherwise and by default, the resource manager of the
    /// current VPE will be cloned.
    pub fn resmng(mut self, rmng: ResMng) -> Self {
        self.rmng = Some(rmng);
        self
    }

    /// Sets the pager. By default, the current pager will be cloned.
    pub fn pager(mut self, pager: Pager) -> Self {
        self.pager = Some(pager);
        self
    }

    /// Sets the kernel memory to use for the VPE. By default, the kernel memory of the current VPE
    /// will be used.
    pub fn kmem(mut self, kmem: Rc<KMem>) -> Self {
        self.kmem = Some(kmem);
        self
    }
}

static CUR: LazyStaticUnsafeCell<VPE> = LazyStaticUnsafeCell::default();

impl VPE {
    fn new_cur() -> Self {
        VPE {
            id: 0,
            cap: Capability::new(kif::SEL_VPE, CapFlags::KEEP_CAP),
            pe: Rc::new(PE::new_bind(0, PEDesc::new_from(0), kif::SEL_PE)),
            rmng: None,
            next_sel: kif::FIRST_FREE_SEL,
            eps_start: 0,
            epmng: EpMng::default(),
            pager: None,
            kmem: Rc::new(KMem::new(kif::SEL_KMEM)),
            files: FileTable::default(),
            mounts: MountTable::default(),
            data: Vec::default(),
        }
    }

    fn init(&mut self) {
        let env = arch::env::get();
        self.id = env.vpe_id();
        self.pe = Rc::new(PE::new_bind(
            env.pe_id() as PEId,
            env.pe_desc(),
            kif::SEL_PE,
        ));
        self.next_sel = env.load_first_sel();
        self.eps_start = env.first_std_ep();
        self.rmng = env.load_rmng();
        self.pager = env.load_pager();
        // mounts first; files depend on mounts
        self.mounts = env.load_mounts();
        self.files = env.load_fds();
        self.data = env.load_data();
        self.epmng.reset();
    }

    /// Puts the current VPE to sleep until the next message arrives
    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::sleep_for(TimeDuration::MAX)
    }

    /// Puts the current VPE to sleep until the next message arrives or `timeout` time has passed.
    #[inline(always)]
    pub fn sleep_for(timeout: TimeDuration) -> Result<(), Error> {
        if envdata::get().platform != envdata::Platform::HOST.val
            && (arch::env::get().shared() || timeout != TimeDuration::MAX)
        {
            let timeout = match timeout {
                TimeDuration::MAX => None,
                t => Some(t),
            };
            return pexif::wait(None, None, timeout);
        }
        if envdata::get().platform != envdata::Platform::HW.val {
            let timeout = match timeout {
                TimeDuration::MAX => None,
                t => Some(t.as_nanos() as u64),
            };
            return TCU::wait_for_msg(INVALID_EP, timeout);
        }
        Ok(())
    }

    /// Puts the current VPE to sleep until the next message arrives on the given EP
    pub fn wait_for(
        ep: Option<EpId>,
        irq: Option<pexif::IRQId>,
        timeout: Option<TimeDuration>,
    ) -> Result<(), Error> {
        if arch::env::get().shared() {
            return pexif::wait(ep, irq, timeout);
        }
        if envdata::get().platform != envdata::Platform::HW.val {
            if let Some(ep) = ep {
                let timeout = timeout.map(|t| t.as_nanos() as u64);
                return TCU::wait_for_msg(ep, timeout);
            }
        }
        Ok(())
    }

    /// Returns the currently running [`VPE`].
    pub fn cur() -> &'static mut VPE {
        CUR.get_mut()
    }

    /// Creates a new `VPE` on PE `pe` with given name and default settings. The VPE provides access
    /// to the PE and allows to run an activity on the PE.
    pub fn new(pe: Rc<PE>, name: &str) -> Result<Self, Error> {
        Self::new_with(pe, VPEArgs::new(name))
    }

    /// Creates a new `VPE` on PE `pe` with given arguments. The VPE provides access to the PE and
    /// allows to run an activity on the PE.
    pub fn new_with(pe: Rc<PE>, args: VPEArgs) -> Result<Self, Error> {
        let sel = VPE::cur().alloc_sels(3);

        let mut vpe = VPE {
            id: 0,
            cap: Capability::new(sel, CapFlags::empty()),
            pe: pe.clone(),
            kmem: args.kmem.unwrap_or_else(|| VPE::cur().kmem.clone()),
            rmng: None,
            next_sel: kif::FIRST_FREE_SEL,
            eps_start: 0,
            epmng: EpMng::default(),
            pager: None,
            files: FileTable::default(),
            mounts: MountTable::default(),
            data: Vec::default(),
        };

        let pager = if vpe.pe.desc().has_virtmem() {
            if let Some(p) = args.pager {
                Some(p)
            }
            else if let Some(p) = Self::cur().pager() {
                Some(p.new_clone()?)
            }
            else {
                None
            }
        }
        else {
            None
        };

        vpe.pager = if let Some(mut pg) = pager {
            // now create VPE, which implicitly obtains the gate cap from us
            let (id, eps_start) = syscalls::create_vpe(sel, args.name, pe.sel(), vpe.kmem.sel())?;
            vpe.id = id;
            vpe.eps_start = eps_start;

            // delegate VPE cap to pager
            pg.init(&vpe)?;
            // and delegate the pager cap to the VPE
            vpe.delegate_obj(pg.sel())?;
            Some(pg)
        }
        else {
            let (id, eps_start) = syscalls::create_vpe(sel, args.name, pe.sel(), vpe.kmem.sel())?;
            vpe.id = id;
            vpe.eps_start = eps_start;
            None
        };
        vpe.next_sel = cmp::max(vpe.kmem.sel() + 1, vpe.next_sel);

        // determine resource manager
        let resmng = if let Some(rmng) = args.rmng {
            vpe.delegate_obj(rmng.sel())?;
            rmng
        }
        else {
            VPE::cur().resmng().unwrap().clone(&mut vpe, &args.name)?
        };
        vpe.rmng = Some(resmng);
        // ensure that the child's cap space is not further ahead than ours
        // TODO improve that
        VPE::cur().next_sel = cmp::max(vpe.next_sel, VPE::cur().next_sel);

        Ok(vpe)
    }

    /// Returns the capability selector.
    pub fn sel(&self) -> Selector {
        self.cap.sel()
    }

    /// Returns the ID of the VPE (for debugging purposes)
    pub fn id(&self) -> VPEId {
        self.id
    }

    /// Returns the description of the PE the VPE has been assigned to.
    pub fn pe(&self) -> &Rc<PE> {
        &self.pe
    }

    /// Returns the description of the PE the VPE has been assigned to.
    pub fn pe_desc(&self) -> PEDesc {
        self.pe.desc()
    }

    /// Returns the id of the PE the VPE has been assigned to.
    pub fn pe_id(&self) -> PEId {
        arch::env::get().pe_id() as PEId
    }

    /// Returns a mutable reference to the file table of this VPE.
    pub fn files(&mut self) -> &mut FileTable {
        &mut self.files
    }

    /// Returns a mutable reference to the mount table of this VPE.
    pub fn mounts(&mut self) -> &mut MountTable {
        &mut self.mounts
    }

    /// Returns a sink for the VPE-local data
    ///
    /// The sink overwrites the VPE-local data and will be transmitted to the VPE when calling
    /// [`VPE::run`] or [`VPE::exec`].
    pub fn data_sink(&mut self) -> StateSerializer {
        StateSerializer::new(&mut self.data)
    }

    /// Returns a source for the VPE-local data
    ///
    /// The source provides access to the VPE-local data that has been transmitted to this VPE from
    /// its parent during [`VPE::run`] or [`VPE::exec`].
    pub fn data_source(&self) -> StateDeserializer {
        StateDeserializer::new(&self.data)
    }

    /// Returns a reference to the VPE's kernel memory.
    pub fn kmem(&self) -> &Rc<KMem> {
        &self.kmem
    }

    /// Returns a reference to the endpoint manager
    pub fn epmng(&self) -> &EpMng {
        &self.epmng
    }

    /// Returns a mutable reference to the endpoint manager
    pub fn epmng_mut(&mut self) -> &mut EpMng {
        &mut self.epmng
    }

    /// Returns a reference to the VPE's resource manager.
    pub fn resmng(&self) -> Option<&ResMng> {
        self.rmng.as_ref()
    }

    /// Returns a reference to the VPE's pager.
    pub fn pager(&self) -> Option<&Pager> {
        self.pager.as_ref()
    }

    /// Allocates a new capability selector and returns it.
    pub fn alloc_sel(&mut self) -> Selector {
        self.alloc_sels(1)
    }

    /// Allocates `count` new and contiguous capability selectors and returns the first one.
    pub fn alloc_sels(&mut self, count: u64) -> Selector {
        self.next_sel += count;
        self.next_sel - count
    }

    /// Delegates the object capability with selector `sel` of [`VPE::cur`] to `self`.
    pub fn delegate_obj(&mut self, sel: Selector) -> Result<(), Error> {
        self.delegate(CapRngDesc::new(CapType::OBJECT, sel, 1))
    }

    /// Delegates the given capability range of [`VPE::cur`] to `self`.
    pub fn delegate(&mut self, crd: CapRngDesc) -> Result<(), Error> {
        let start = crd.start();
        self.delegate_to(crd, start)
    }

    /// Delegates the given capability range of [`VPE::cur`] to `self` using selectors
    /// `dst`..`dst`+`crd.count()`.
    pub fn delegate_to(&mut self, crd: CapRngDesc, dst: Selector) -> Result<(), Error> {
        syscalls::exchange(self.sel(), crd, dst, false)?;
        self.next_sel = cmp::max(self.next_sel, dst + crd.count());
        Ok(())
    }

    /// Obtains the object capability with selector `sel` from `self` to [`VPE::cur`].
    pub fn obtain_obj(&mut self, sel: Selector) -> Result<Selector, Error> {
        self.obtain(CapRngDesc::new(CapType::OBJECT, sel, 1))
    }

    /// Obtains the given capability range of `self` to [`VPE::cur`].
    pub fn obtain(&mut self, crd: CapRngDesc) -> Result<Selector, Error> {
        let count = crd.count();
        let start = VPE::cur().alloc_sels(count);
        self.obtain_to(crd, start).map(|_| start)
    }

    /// Obtains the given capability range of `self` to [`VPE::cur`] using selectors
    /// `dst`..`dst`+`crd.count()`.
    pub fn obtain_to(&mut self, crd: CapRngDesc, dst: Selector) -> Result<(), Error> {
        let own = CapRngDesc::new(crd.cap_type(), dst, crd.count());
        syscalls::exchange(self.sel(), own, crd.start(), true)
    }

    /// Revokes the given capability range from `self`.
    ///
    /// If `del_only` is true, only the delegations are revoked, that is, the capability is not
    /// revoked from `self`.
    pub fn revoke(&mut self, crd: CapRngDesc, del_only: bool) -> Result<(), Error> {
        syscalls::revoke(self.sel(), crd, !del_only)
    }

    /// Performs the required capability exchanges to pass the files set for `self` to the VPE.
    ///
    /// Before calling this method, you should adjust the file table of `self` via [`VPE::files`]
    /// by copying files from [`VPE::cur`].
    pub fn obtain_fds(&mut self) -> Result<(), Error> {
        // TODO that's really bad. but how to improve that? :/
        let mut dels = Vec::new();
        self.files
            .collect_caps(self.sel(), &mut dels, &mut self.next_sel)?;
        for c in dels {
            self.delegate_obj(c)?;
        }
        Ok(())
    }

    /// Performs the required capability exchanges to pass the mounts set for `self` to the VPE.
    ///
    /// Before calling this method, you should adjust the mount table of `self` via [`VPE::mounts`]
    /// by copying mounts from [`VPE::cur`].
    pub fn obtain_mounts(&mut self) -> Result<(), Error> {
        let mut dels = Vec::new();
        self.mounts
            .collect_caps(self.sel(), &mut dels, &mut self.next_sel)?;
        for c in dels {
            self.delegate_obj(c)?;
        }
        Ok(())
    }

    /// Creates a new memory gate that refers to the address region `addr`..`addr`+`size` in the
    /// address space of this VPE. The region must be physically contiguous and page aligned.
    pub fn get_mem(&self, addr: goff, size: goff, perms: kif::Perm) -> Result<MemGate, Error> {
        MemGate::new_foreign(self.sel(), addr, size, perms)
    }

    /// Starts the VPE without running any code on it. This is intended for non-programmable
    /// accelerators and devices that implement the PEMux protocol to get started, but don't execute
    /// any code.
    pub fn start(self) -> Result<DeviceActivity, Error> {
        use crate::pes::Activity;

        let act = DeviceActivity::new(self);
        act.start().map(|_| act)
    }

    /// Executes the program of `VPE::cur()` (`argv[0]`) on this VPE and calls the given function
    /// instead of main.
    ///
    /// This has a few requirements/limitations:
    /// 1. the current binary has to be stored in a file system
    /// 2. this file system needs to be mounted for this VPE, such that `argv[0]` is the current binary
    ///
    /// The method returns the [`ExecActivity`] on success that can be used to wait for the
    /// functions completeness or to stop it.
    pub fn run(self, func: fn() -> i32) -> Result<ExecActivity, Error> {
        let args = env::args().collect::<Vec<_>>();
        let file = VFS::open(args[0].as_ref(), OpenFlags::RX | OpenFlags::NEW_SESS)?;
        let mut mapper = DefaultMapper::new(self.pe_desc().has_virtmem());

        let func_addr = func as *const () as usize;
        self.do_exec_file(&mut mapper, file, &args, Some(func_addr))
    }

    /// Executes the given program and arguments on `self`.
    ///
    /// The method returns the [`ExecActivity`] on success that can be used to wait for the
    /// program completeness or to stop it.
    pub fn exec<S: AsRef<str>>(self, args: &[S]) -> Result<ExecActivity, Error> {
        let file = VFS::open(args[0].as_ref(), OpenFlags::RX | OpenFlags::NEW_SESS)?;
        let mut mapper = DefaultMapper::new(self.pe_desc().has_virtmem());
        self.exec_file(&mut mapper, file, args)
    }

    /// Executes the program given as a [`FileRef`] on `self`, using `mapper` to initiate the
    /// address space and `args` as the arguments.
    ///
    /// The file has to have its own file session and therefore needs to be opened with
    /// OpenFlags::NEW_SESS.
    ///
    /// The method returns the [`ExecActivity`] on success that can be used to wait for the
    /// program completeness or to stop it.
    pub fn exec_file<S: AsRef<str>>(
        self,
        mapper: &mut dyn Mapper,
        file: FileRef,
        args: &[S],
    ) -> Result<ExecActivity, Error> {
        self.do_exec_file(mapper, file, args, None)
    }

    #[cfg(not(target_vendor = "host"))]
    #[allow(unused_mut)]
    fn do_exec_file<S: AsRef<str>>(
        self,
        mapper: &mut dyn Mapper,
        mut file: FileRef,
        args: &[S],
        closure: Option<usize>,
    ) -> Result<ExecActivity, Error> {
        use crate::cfg;
        use crate::mem;
        use crate::pes::Activity;

        let mut file = BufReader::new(file);

        let mut senv = arch::env::EnvData::default();

        let env_page_off = (cfg::ENV_START & !cfg::PAGE_MASK) as goff;
        let mem = self.get_mem(env_page_off, cfg::ENV_SIZE as goff, kif::Perm::RW)?;

        {
            // load program segments
            senv.set_platform(arch::env::get().platform());
            senv.set_sp(self.pe_desc().stack_top());
            senv.set_entry(arch::loader::load_program(&self, mapper, &mut file)?);

            // write args
            let mut off = cfg::ENV_START + mem::size_of_val(&senv);
            senv.set_argc(args.len());
            senv.set_argv(arch::loader::write_arguments(&mem, &mut off, args)?);

            // write file table
            {
                let mut fds_vec = Vec::new();
                let mut fds = StateSerializer::new(&mut fds_vec);
                self.files.serialize(&mut fds);
                let words = fds.words();
                mem.write_bytes(
                    words.as_ptr() as *const u8,
                    words.len() * mem::size_of::<u64>(),
                    off as goff - env_page_off,
                )?;
                senv.set_files(off, fds.size());
                off += fds.size();
            }

            // write mounts table
            {
                let mut mounts_vec = Vec::new();
                let mut mounts = StateSerializer::new(&mut mounts_vec);
                self.mounts.serialize(&mut mounts);
                let words = mounts.words();
                mem.write_bytes(
                    words.as_ptr() as *const u8,
                    words.len() * mem::size_of::<u64>(),
                    off as goff - env_page_off,
                )?;
                senv.set_mounts(off, mounts.size());
                off += mounts.size();
            }

            // write data
            {
                let size = self.data.len() * mem::size_of::<u64>();
                mem.write_bytes(
                    self.data.as_ptr() as *const u8,
                    size,
                    off as goff - env_page_off,
                )?;
                senv.set_data(off, size);
            }

            // write closure
            if let Some(addr) = closure {
                senv.set_closure(addr);
            }

            senv.set_first_std_ep(self.eps_start);
            senv.set_rmng(self.resmng().unwrap().sel());
            senv.set_first_sel(self.next_sel);
            senv.set_pedesc(self.pe_desc());
            senv.set_vpe_id(self.id());

            if let Some(ref pg) = self.pager {
                senv.set_pager(pg);
                senv.set_heap_size(cfg::APP_HEAP_SIZE);
            }
            else {
                senv.set_heap_size(cfg::MOD_HEAP_SIZE);
            }

            // write start env to PE
            mem.write_bytes(
                &senv as *const _ as *const u8,
                mem::size_of_val(&senv),
                cfg::ENV_START as goff - env_page_off,
            )?;
        }

        // go!
        let act = ExecActivity::new(self, file);
        act.start().map(|_| act)
    }

    #[cfg(target_vendor = "host")]
    fn do_exec_file<S: AsRef<str>>(
        self,
        _mapper: &dyn Mapper,
        mut file: FileRef,
        args: &[S],
        closure: Option<usize>,
    ) -> Result<ExecActivity, Error> {
        use crate::errors::Code;
        use crate::libc;

        let path = arch::loader::copy_file(&mut file)?;

        let mut p2c = arch::loader::Channel::new()?;
        let mut c2p = arch::loader::Channel::new()?;

        match unsafe { libc::fork() } {
            -1 => Err(Error::new(Code::OutOfMem)),

            0 => {
                // wait until the env file has been written by the kernel
                p2c.wait();

                let pid = unsafe { libc::getpid() };

                // tell child about fd to notify parent if TCU is ready
                arch::loader::write_env_values(pid, "tcurdy", &[c2p.fds()[1] as u64]);

                // write nextsel, eps, rmng, and kmem
                arch::loader::write_env_values(pid, "nextsel", &[u64::from(self.next_sel)]);
                arch::loader::write_env_values(pid, "rmng", &[u64::from(
                    self.resmng().unwrap().sel(),
                )]);
                arch::loader::write_env_values(pid, "kmem", &[u64::from(self.kmem.sel())]);

                // write closure
                if let Some(addr) = closure {
                    arch::loader::write_env_values(pid, "lambda", &[addr as u64]);
                }

                // write file table
                let mut fds_vec = Vec::new();
                let mut fds = StateSerializer::new(&mut fds_vec);
                self.files.serialize(&mut fds);
                arch::loader::write_env_values(pid, "fds", fds.words());

                // write mounts table
                let mut mounts_vec = Vec::new();
                let mut mounts = StateSerializer::new(&mut mounts_vec);
                self.mounts.serialize(&mut mounts);
                arch::loader::write_env_values(pid, "ms", mounts.words());

                // write data
                arch::loader::write_env_values(pid, "data", &self.data);

                arch::loader::exec(args, &path);
            },

            pid => {
                // let the kernel create the config-file etc. for the given pid
                syscalls::vpe_ctrl(self.sel(), kif::syscalls::VPEOp::START, pid as u64).unwrap();

                p2c.signal();
                // wait until the TCU sockets have been binded
                c2p.wait();

                Ok(ExecActivity::new(self, BufReader::new(file)))
            },
        }
    }
}

impl fmt::Debug for VPE {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "VPE[sel: {}, pe: {:?}]", self.sel(), self.pe())
    }
}

pub(crate) fn init() {
    CUR.set(VPE::new_cur());
    VPE::cur().init();
}
