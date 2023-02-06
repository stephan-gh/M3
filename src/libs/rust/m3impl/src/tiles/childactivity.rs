/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

//! Contains the abstraction for child activities

use core::cmp;
use core::fmt;
use core::ops::{Deref, DerefMut};

use crate::cap::{CapFlags, Capability, Selector};
use crate::cell::Cell;
use crate::cfg;
use crate::col::{String, ToString, Vec};
use crate::com::MemGate;
use crate::env::Env;
use crate::errors::Error;
use crate::goff;
use crate::kif;
use crate::kif::{CapRngDesc, CapType};
use crate::mem;
use crate::rc::Rc;
use crate::serialize::{M3Serializer, VecSink};
use crate::session::{Pager, ResMng};
use crate::syscalls;
use crate::tiles::{
    loader, Activity, DefaultMapper, KMem, Mapper, RunningActivity, RunningDeviceActivity,
    RunningProgramActivity, Tile,
};
use crate::util::math;
use crate::vfs::{BufReader, Fd, File, FileRef, OpenFlags, VFS};

/// Represents a child activity.
pub struct ChildActivity {
    base: Activity,
    child_sel: Cell<Selector>,
    files: Vec<(Fd, Fd)>,
    mounts: Vec<(String, String)>,
}

/// The arguments for [`ChildActivity`] creations.
pub struct ActivityArgs<'n> {
    name: &'n str,
    pager: Option<Pager>,
    kmem: Option<Rc<KMem>>,
    rmng: Option<ResMng>,
    first_sel: Selector,
}

impl<'n> ActivityArgs<'n> {
    /// Creates a new instance of `ActivityArgs` using default settings.
    pub fn new(name: &'n str) -> ActivityArgs<'n> {
        ActivityArgs {
            name,
            pager: None,
            kmem: None,
            rmng: None,
            first_sel: kif::FIRST_FREE_SEL,
        }
    }

    /// Sets the resource manager to `rmng`. Otherwise and by default, the resource manager of the
    /// own activity will be cloned.
    pub fn resmng(mut self, rmng: ResMng) -> Self {
        self.rmng = Some(rmng);
        self
    }

    /// Sets the pager. By default, the own pager will be cloned.
    pub fn pager(mut self, pager: Pager) -> Self {
        self.pager = Some(pager);
        self
    }

    /// Sets the kernel memory to use for the activity. By default, the kernel memory of the own
    /// activity will be used.
    pub fn kmem(mut self, kmem: Rc<KMem>) -> Self {
        self.kmem = Some(kmem);
        self
    }

    /// Sets the first selector to be used by the child (kif::FIRST_FREE_SEL by default).
    pub fn first_sel(mut self, sel: Selector) -> Self {
        self.first_sel = sel;
        self
    }
}

impl ChildActivity {
    /// Creates a new [`ChildActivity`] on tile `tile` with given name and default settings. The
    /// activity provides access to the tile and allows to run an activity on the tile.
    pub fn new(tile: Rc<Tile>, name: &str) -> Result<Self, Error> {
        Self::new_with(tile, ActivityArgs::new(name))
    }

    /// Creates a new [`ChildActivity`] on tile `tile` with given arguments. The activity provides
    /// access to the tile and allows to run an activity on the tile.
    pub fn new_with(tile: Rc<Tile>, args: ActivityArgs<'_>) -> Result<Self, Error> {
        let sel = Activity::own().alloc_sels(3);

        // create child activity struct
        let mut act = ChildActivity {
            base: Activity::new_act(
                Capability::new(sel, CapFlags::empty()),
                tile.clone(),
                args.kmem.unwrap_or_else(|| Activity::own().kmem().clone()),
            ),
            child_sel: Cell::from(args.first_sel),
            files: Vec::new(),
            mounts: Vec::new(),
        };

        // determine pager
        let pager = if act.tile_desc().has_virtmem() {
            if let Some(p) = args.pager {
                Some(p)
            }
            else if let Some(p) = Activity::own().pager() {
                Some(p.new_clone()?)
            }
            else {
                None
            }
        }
        else {
            None
        };

        // actually create activity via syscall
        let (id, eps_start) =
            syscalls::create_activity(sel, args.name, tile.sel(), act.kmem().sel())?;
        act.id = id;
        act.eps_start = eps_start;

        // initialize pager
        act.pager = if let Some(mut pg) = pager {
            pg.init(&act)?;
            Some(pg)
        }
        else {
            None
        };

        act.child_sel
            .set(cmp::max(act.kmem().sel() + 1, act.child_sel.get()));

        // determine resource manager
        act.rmng = if let Some(rmng) = args.rmng {
            act.delegate_obj(rmng.sel())?;
            Some(rmng)
        }
        else {
            let sgate_sel = act.child_sel.get();
            act.child_sel.set(sgate_sel + 1);

            Some(
                Activity::own()
                    .resmng()
                    .unwrap()
                    .clone(&mut act, sgate_sel, args.name)?,
            )
        };

        // ensure that the child's cap space is not further ahead than ours
        // TODO improve that
        Activity::own().next_sel.set(cmp::max(
            act.child_sel.get(),
            Activity::own().next_sel.get(),
        ));

        Ok(act)
    }

    /// Returns the selector of the resource manager
    pub fn resmng_sel(&self) -> Option<Selector> {
        self.rmng.as_ref().map(|r| r.sel())
    }

    /// Returns the map of files (destination fd, source fd) that are going to be delegated to this
    /// child activity on [`run`](Activity::run) and [`exec`](Activity::exec).
    pub(crate) fn files(&self) -> &Vec<(Fd, Fd)> {
        &self.files
    }

    /// Returns the map of mounts (destination path, source path) that are going to be delegated to
    /// this child activity on [`run`](Activity::run) and [`exec`](Activity::exec).
    pub(crate) fn mounts(&self) -> &Vec<(String, String)> {
        &self.mounts
    }

    /// Installs file `our_fd` as `child_fd` in this child activity.
    ///
    /// Files that are added to child activities are automatically delegated to the child upon
    /// [`run`](ChildActivity::run) and [`exec`](ChildActivity::exec).
    pub fn add_file(&mut self, child_fd: Fd, our_fd: Fd) {
        if let Some(mapping) = self.files.iter_mut().find(|(c, _p)| *c == child_fd) {
            mapping.1 = our_fd;
        }
        else {
            self.files.push((child_fd, our_fd));
        }
    }

    /// Installs mount `our_path` as `child_path` in this child activity.
    ///
    /// Mounts that are added to child activities are automatically delegated to the child upon
    /// [`run`](ChildActivity::run) and [`exec`](ChildActivity::exec).
    pub fn add_mount(&mut self, child_path: &str, our_path: &str) {
        if let Some(mapping) = self.mounts.iter_mut().find(|(c, _p)| c == child_path) {
            mapping.1 = our_path.to_string();
        }
        else {
            self.mounts
                .push((child_path.to_string(), our_path.to_string()))
        }
    }

    /// Returns a sink for the activity-local data
    ///
    /// The sink overwrites the activity-local data and will be transmitted to the activity when calling
    /// [`run`](ChildActivity::run) and [`exec`](ChildActivity::exec).
    pub fn data_sink(&mut self) -> M3Serializer<VecSink<'_>> {
        M3Serializer::new(VecSink::new(&mut self.data))
    }

    /// Delegates the object capability with selector `sel` of [`Activity::own`](Activity::own) to
    /// `self`.
    pub fn delegate_obj(&self, sel: Selector) -> Result<(), Error> {
        self.delegate(CapRngDesc::new(CapType::OBJECT, sel, 1))
    }

    /// Delegates the given capability range of [`Activity::own`](Activity::own) to `self`.
    pub fn delegate(&self, crd: CapRngDesc) -> Result<(), Error> {
        let start = crd.start();
        self.delegate_to(crd, start)
    }

    /// Delegates the given capability range of [`Activity::own`](Activity::own) to `self` using
    /// selectors `dst`..`dst`+`crd.count()`.
    pub fn delegate_to(&self, crd: CapRngDesc, dst: Selector) -> Result<(), Error> {
        syscalls::exchange(self.sel(), crd, dst, false)?;
        self.child_sel
            .set(cmp::max(self.child_sel.get(), dst + crd.count()));
        Ok(())
    }

    /// Obtains the object capability with selector `sel` from `self` to
    /// [`Activity::own`](Activity::own).
    pub fn obtain_obj(&self, sel: Selector) -> Result<Selector, Error> {
        self.obtain(CapRngDesc::new(CapType::OBJECT, sel, 1))
    }

    /// Obtains the given capability range of `self` to [`Activity::own`](Activity::own).
    pub fn obtain(&self, crd: CapRngDesc) -> Result<Selector, Error> {
        let count = crd.count();
        let start = Activity::own().alloc_sels(count);
        self.obtain_to(crd, start).map(|_| start)
    }

    /// Obtains the given capability range of `self` to [`Activity::own`](Activity::own) using
    /// selectors `dst`..`dst`+`crd.count()`.
    pub fn obtain_to(&self, crd: CapRngDesc, dst: Selector) -> Result<(), Error> {
        let own = CapRngDesc::new(crd.cap_type(), dst, crd.count());
        syscalls::exchange(self.sel(), own, crd.start(), true)
    }

    /// Starts the activity without running any code on it. This is intended for non-programmable
    /// accelerators and devices that implement the TileMux protocol to get started, but don't
    /// execute any code.
    pub fn start(self) -> Result<RunningDeviceActivity, Error> {
        let act = RunningDeviceActivity::new(self);
        act.start().map(|_| act)
    }

    /// Executes the program of [`Activity::own`](Activity::own) (`argv[0]`) with this activity and
    /// calls the given function instead of main.
    ///
    /// This has a few requirements/limitations:
    /// 1. the current binary has to be stored in a file system
    /// 2. this file system needs to be mounted, such that `argv[0]` is the current binary
    ///
    /// The method returns the [`RunningProgramActivity`] on success that can be used to wait for
    /// the functions completeness or to stop it.
    pub fn run(self, func: fn() -> Result<(), Error>) -> Result<RunningProgramActivity, Error> {
        let args = crate::env::args().collect::<Vec<_>>();
        let file = VFS::open(args[0], OpenFlags::RX | OpenFlags::NEW_SESS)?;
        let mut mapper = DefaultMapper::new(self.tile_desc().has_virtmem());

        let func_addr = func as *const () as usize;
        self.do_exec_file(&mut mapper, file.into_generic(), &args, Some(func_addr))
    }

    /// Executes the given program and arguments with `self`.
    ///
    /// The method returns the [`RunningProgramActivity`] on success that can be used to wait for
    /// the program completeness or to stop it.
    pub fn exec<S: AsRef<str>>(self, args: &[S]) -> Result<RunningProgramActivity, Error> {
        let file = VFS::open(args[0].as_ref(), OpenFlags::RX | OpenFlags::NEW_SESS)?;
        let mut mapper = DefaultMapper::new(self.tile_desc().has_virtmem());
        self.exec_file(&mut mapper, file.into_generic(), args)
    }

    /// Executes the program given as a [`FileRef`] with `self`, using `mapper` to initiate the
    /// address space and `args` as the arguments.
    ///
    /// The file has to have its own file session and therefore needs to be opened with
    /// [`OpenFlags::NEW_SESS`].
    ///
    /// The method returns the [`RunningProgramActivity`] on success that can be used to wait for
    /// the program completeness or to stop it.
    pub fn exec_file<S: AsRef<str>>(
        self,
        mapper: &mut dyn Mapper,
        file: FileRef<dyn File>,
        args: &[S],
    ) -> Result<RunningProgramActivity, Error> {
        self.do_exec_file(mapper, file, args, None)
    }

    fn do_exec_file<S: AsRef<str>>(
        self,
        mapper: &mut dyn Mapper,
        file: FileRef<dyn File>,
        args: &[S],
        closure: Option<usize>,
    ) -> Result<RunningProgramActivity, Error> {
        self.obtain_files_and_mounts()?;

        let mut file = BufReader::new(file);
        let entry = loader::load_program(&self, mapper, &mut file)?;

        self.load_environment(args, closure, entry)?;

        let act = RunningProgramActivity::new(self, file);
        act.start().map(|_| act)
    }

    fn obtain_files_and_mounts(&self) -> Result<(), Error> {
        let fsel = Activity::own().files().delegate(self)?;
        let msel = Activity::own().mounts().delegate(self)?;
        self.child_sel.set(self.child_sel.get().max(msel.max(fsel)));
        Ok(())
    }

    fn load_environment<S: AsRef<str>>(
        &self,
        args: &[S],
        closure: Option<usize>,
        entry: usize,
    ) -> Result<(), Error> {
        let env_page_off = (cfg::ENV_START & !cfg::PAGE_MASK) as goff;
        let mem = self.get_mem(env_page_off, cfg::ENV_SIZE as goff, kif::Perm::RW)?;

        // build child environment
        let mut cenv = crate::env::Env::default();
        cenv.set_platform(crate::env::get().platform());
        cenv.set_sp(self.tile_desc().stack_top());
        cenv.set_entry(entry);
        cenv.set_first_std_ep(self.eps_start);
        cenv.set_rmng(self.resmng_sel().unwrap());
        cenv.set_first_sel(self.child_sel.get());
        cenv.set_pedesc(self.tile_desc());
        cenv.set_activity_id(self.id());
        cenv.copy_tile_ids(crate::env::get().tile_ids());

        if let Some(addr) = closure {
            cenv.set_closure(addr);
        }

        if let Some(ref pg) = self.pager {
            cenv.set_pager(pg);
            cenv.set_heap_size(cfg::APP_HEAP_SIZE);
        }
        else {
            cenv.set_heap_size(cfg::MOD_HEAP_SIZE);
        }

        // write arguments and environment variables
        let mut off = cfg::ENV_START + mem::size_of_val(&cenv);
        cenv.set_argc(args.len());
        cenv.set_argv(Self::write_arguments(args, &mem, &mut off)?);
        cenv.set_envp(Self::write_arguments(
            &crate::env::vars_raw(),
            &mem,
            &mut off,
        )?);

        // serialize files, mounts, and data and write them to the child's memory
        let write_words = |words: &[u64], off| mem.write(words, off as goff - env_page_off);
        self.serialize_files(write_words, &mut cenv, &mut off)?;
        self.serialize_mounts(write_words, &mut cenv, &mut off)?;
        self.serialize_data(write_words, &mut cenv, &mut off)?;

        // write environment to tile
        mem.write_bytes(
            &cenv as *const _ as *const u8,
            mem::size_of_val(&cenv),
            cfg::ENV_START as goff - env_page_off,
        )
    }

    fn write_arguments<S>(args: &[S], mem: &MemGate, off: &mut usize) -> Result<usize, Error>
    where
        S: AsRef<str>,
    {
        let (arg_buf, arg_ptr, arg_end) = Self::collect_args(args, *off);

        // write actual arguments to memory
        let env_page_off = (cfg::ENV_START & !cfg::PAGE_MASK) as goff;
        mem.write_bytes(
            arg_buf.as_ptr() as *const _,
            arg_buf.len(),
            *off as goff - env_page_off,
        )?;

        // write argument pointers to memory
        let arg_ptr_off = math::round_up(arg_end, mem::size_of::<u64>());
        mem.write_bytes(
            arg_ptr.as_ptr() as *const _,
            arg_ptr.len() * mem::size_of::<u64>(),
            arg_ptr_off as goff - env_page_off,
        )?;

        *off = arg_ptr_off + arg_ptr.len() * mem::size_of::<u64>();
        Ok(arg_ptr_off)
    }

    fn collect_args<S>(args: &[S], off: usize) -> (Vec<u8>, Vec<u64>, usize)
    where
        S: AsRef<str>,
    {
        let mut arg_ptr = Vec::<u64>::new();
        let mut arg_buf = Vec::new();

        let mut arg_off = off;
        for s in args {
            // push argv entry
            arg_ptr.push(arg_off as u64);

            // push string
            let arg = s.as_ref().as_bytes();
            arg_buf.extend_from_slice(arg);

            // 0-terminate it
            arg_buf.push(b'\0');

            arg_off += arg.len() + 1;
        }
        arg_ptr.push(0);

        (arg_buf, arg_ptr, arg_off)
    }

    fn serialize_files<F>(&self, write: F, env: &mut Env, off: &mut usize) -> Result<(), Error>
    where
        F: Fn(&[u64], usize) -> Result<(), Error>,
    {
        let mut fds_vec = Vec::new();
        let mut fds = M3Serializer::new(VecSink::new(&mut fds_vec));
        Activity::own().files().serialize(&self.files, &mut fds);
        write(fds.words(), *off)?;
        env.set_files(*off, fds.size());
        *off += fds.size();
        Ok(())
    }

    fn serialize_mounts<F>(&self, write: F, env: &mut Env, off: &mut usize) -> Result<(), Error>
    where
        F: Fn(&[u64], usize) -> Result<(), Error>,
    {
        let mut mounts_vec = Vec::new();
        let mut mounts = M3Serializer::new(VecSink::new(&mut mounts_vec));
        Activity::own()
            .mounts()
            .serialize(&self.mounts, &mut mounts);
        write(mounts.words(), *off)?;
        env.set_mounts(*off, mounts.size());
        *off += mounts.size();
        Ok(())
    }

    fn serialize_data<F>(&self, write: F, env: &mut Env, off: &mut usize) -> Result<(), Error>
    where
        F: Fn(&[u64], usize) -> Result<(), Error>,
    {
        write(&self.data, *off)?;
        env.set_data(*off, self.data.len() * mem::size_of::<u64>());
        Ok(())
    }
}

impl Deref for ChildActivity {
    type Target = Activity;

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.base
    }
}

impl DerefMut for ChildActivity {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl fmt::Debug for ChildActivity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "ChildActivity[sel: {}, tile: {:?}]",
            self.sel(),
            self.tile()
        )
    }
}
