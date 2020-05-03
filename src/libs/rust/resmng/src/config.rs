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

use core::fmt;
use m3::cap::Selector;
use m3::cell::Cell;
use m3::cell::RefCell;
use m3::col::{BTreeSet, String, Vec};
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif;
use m3::rc::Rc;

use parser;
use pes;

#[derive(Default)]
pub struct PhysMemDesc {
    phys: goff,
    size: goff,
}

impl PhysMemDesc {
    pub(crate) fn new(phys: goff, size: goff) -> Self {
        PhysMemDesc { phys, size }
    }

    pub fn phys(&self) -> goff {
        self.phys
    }

    pub fn size(&self) -> goff {
        self.size
    }
}

#[derive(Default)]
pub struct ServiceDesc {
    local_name: String,
    global_name: String,
    used: RefCell<bool>,
}

impl ServiceDesc {
    pub(crate) fn new(local_name: String, global_name: String) -> Self {
        ServiceDesc {
            local_name,
            global_name,
            used: RefCell::new(false),
        }
    }

    pub fn global_name(&self) -> &String {
        &self.global_name
    }

    pub fn is_used(&self) -> bool {
        *self.used.borrow()
    }

    pub fn mark_used(&self) {
        self.used.replace(true);
    }
}

#[derive(Default)]
pub struct SessionDesc {
    local_name: String,
    serv: String,
    arg: String,
    usage: RefCell<Option<Selector>>,
}

impl SessionDesc {
    pub(crate) fn new(local_name: String, serv: String, arg: String) -> Self {
        SessionDesc {
            local_name,
            serv,
            arg,
            usage: RefCell::new(None),
        }
    }

    pub fn serv_name(&self) -> &String {
        &self.serv
    }

    pub fn arg(&self) -> &String {
        &self.arg
    }

    pub fn is_used(&self) -> bool {
        self.usage.borrow().is_some()
    }

    pub fn mark_used(&self, sel: Selector) {
        self.usage.replace(Some(sel));
    }
}

#[derive(Default)]
pub struct PEDesc {
    ty: String,
    count: Cell<u32>,
    optional: bool,
}

impl PEDesc {
    pub(crate) fn new(ty: String, count: u32, optional: bool) -> Self {
        PEDesc {
            ty,
            count: Cell::new(count),
            optional,
        }
    }

    pub fn pe_type(&self) -> &String {
        &self.ty
    }

    pub fn matches(&self, desc: kif::PEDesc) -> bool {
        match self.ty.as_ref() {
            "core" => desc.is_programmable(),
            "arm" => desc.isa() == kif::PEISA::ARM,
            "x86" => desc.isa() == kif::PEISA::X86,
            "indir" => desc.isa() == kif::PEISA::ACCEL_INDIR,
            "copy" => desc.isa() == kif::PEISA::ACCEL_COPY,
            "rot13" => desc.isa() == kif::PEISA::ACCEL_ROT13,
            "ide" => desc.isa() == kif::PEISA::IDE_DEV,
            "nic" => desc.isa() == kif::PEISA::NIC_DEV,
            _ => false,
        }
    }

    pub fn alloc(&self) {
        assert!(self.count.get() > 0);
        self.count.set(self.count.get() - 1);
    }

    pub fn free(&self) {
        self.count.set(self.count.get() + 1);
    }
}

pub struct SemDesc {
    local_name: String,
    global_name: String,
}

impl SemDesc {
    pub(crate) fn new(local_name: String, global_name: String) -> Self {
        SemDesc {
            local_name,
            global_name,
        }
    }

    pub fn global_name(&self) -> &String {
        &self.global_name
    }
}

pub struct Config {
    pub(crate) doms: Vec<Domain>,
}

impl Config {
    pub fn parse(xml: &str, restrict: bool) -> Result<Self, Error> {
        parser::parse(xml, restrict)
    }

    pub fn domains(&self) -> &Vec<Domain> {
        &self.doms
    }

    pub fn count_apps(&self) -> usize {
        self.doms.iter().fold(0, |total, d| total + d.apps.len())
    }

    pub fn check(&self) {
        let mut services = BTreeSet::new();
        for d in &self.doms {
            for a in &d.apps {
                Self::collect_services(&mut services, a);
            }
        }

        for d in &self.doms {
            for a in &d.apps {
                Self::check_services(&services, a);
            }
        }

        for d in &self.doms {
            for a in &d.apps {
                Self::check_pes(a);
            }
        }
    }

    fn count_pes(pe: &PEDesc) -> u32 {
        let mut count = 0;
        for i in 0..pes::get().count() {
            if pe.matches(pes::get().get(i).desc()) {
                count += 1;
            }
        }
        count
    }

    fn check_pes(app: &AppConfig) {
        for pe in &app.pes {
            if !pe.optional {
                let available = Self::count_pes(&pe);
                if available < pe.count.get() {
                    panic!(
                        "AppConfig '{}' needs PE type '{}' {} times, but {} are available",
                        app.name(),
                        pe.ty,
                        pe.count.get(),
                        available
                    );
                }
            }
        }
    }

    fn collect_services(set: &mut BTreeSet<String>, app: &AppConfig) {
        for serv in app.services() {
            if set.contains(serv.global_name()) {
                panic!(
                    "config '{}': service '{}' does already exist",
                    app.name(),
                    serv.global_name()
                );
            }
            set.insert(serv.global_name().clone());
        }
    }

    fn check_services(set: &BTreeSet<String>, app: &AppConfig) {
        for sess in app.sessions() {
            if !set.contains(sess.serv_name()) {
                panic!(
                    "config '{}': service '{}' does not exist",
                    app.name(),
                    sess.serv_name()
                );
            }
        }
    }
}

#[derive(Default)]
pub struct Domain {
    pub(crate) apps: Vec<Rc<AppConfig>>,
}

impl Domain {
    pub fn apps(&self) -> &Vec<Rc<AppConfig>> {
        &self.apps
    }
}

#[derive(Default)]
pub struct AppConfig {
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
    pub(crate) restrict: bool,
    pub(crate) daemon: bool,
    pub(crate) user_mem: Option<usize>,
    pub(crate) kern_mem: Option<usize>,
    pub(crate) phys_mems: Vec<PhysMemDesc>,
    pub(crate) services: Vec<ServiceDesc>,
    pub(crate) sessions: Vec<SessionDesc>,
    pub(crate) sems: Vec<SemDesc>,
    pub(crate) pes: Vec<PEDesc>,
}

impl AppConfig {
    pub fn new(args: Vec<String>, restrict: bool) -> Self {
        assert!(!args.is_empty());
        let mut cfg = AppConfig::default();
        cfg.name = args[0].clone();
        cfg.args = args;
        cfg.restrict = restrict;
        cfg
    }

    pub fn daemon(&self) -> bool {
        self.daemon
    }

    pub fn restrict(&self) -> bool {
        self.restrict
    }

    pub fn user_mem(&self) -> Option<usize> {
        self.user_mem
    }

    pub fn kernel_mem(&self) -> Option<usize> {
        self.kern_mem
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn args(&self) -> &Vec<String> {
        &self.args
    }

    pub fn phys_mems(&self) -> &Vec<PhysMemDesc> {
        &self.phys_mems
    }

    pub fn services(&self) -> &Vec<ServiceDesc> {
        &self.services
    }

    pub fn sessions(&self) -> &Vec<SessionDesc> {
        &self.sessions
    }

    pub fn get_sem(&self, lname: &str) -> Option<&SemDesc> {
        self.sems.iter().find(|s| s.local_name == *lname)
    }

    pub fn get_service(&self, lname: &str) -> Option<&ServiceDesc> {
        self.services.iter().find(|s| s.local_name == *lname)
    }

    pub fn unreg_service(&self, gname: &str) {
        if !self.restrict {
            return;
        }

        let serv = self
            .services
            .iter()
            .find(|s| s.global_name == *gname)
            .unwrap();
        serv.used.replace(false);
    }

    pub fn get_session(&self, lname: &str) -> Option<&SessionDesc> {
        self.sessions.iter().find(|s| s.local_name == *lname)
    }

    pub fn close_session(&self, sel: Selector) {
        if !self.restrict {
            return;
        }

        let sess = self
            .sessions
            .iter()
            .find(|s| {
                if let Some(s) = *s.usage.borrow() {
                    s == sel
                }
                else {
                    false
                }
            })
            .unwrap();
        sess.usage.replace(None);
    }

    pub fn get_pe_idx(&self, desc: kif::PEDesc) -> Result<usize, Error> {
        let idx = self
            .pes
            .iter()
            .position(|pe| pe.matches(desc))
            .ok_or_else(|| Error::new(Code::InvArgs))?;

        if self.pes[idx].count.get() > 0 {
            Ok(idx)
        }
        else {
            Err(Error::new(Code::NoPerm))
        }
    }

    pub fn alloc_pe(&self, idx: usize) {
        self.pes[idx].alloc();
    }

    pub fn free_pe(&self, idx: usize) {
        self.pes[idx].free();
    }

    fn print_rec(&self, f: &mut fmt::Formatter, layer: usize) -> Result<(), fmt::Error> {
        write!(f, "{:0w$}", "", w = layer)?;
        for a in &self.args {
            write!(f, "{} ", a)?;
        }
        writeln!(f, "[")?;
        if self.daemon {
            writeln!(f, "{:0w$}Daemon,", "", w = layer + 2)?;
        }
        if let Some(umem) = self.user_mem {
            writeln!(
                f,
                "{:0w$}UserMem[size={} KiB],",
                "",
                umem / 1024,
                w = layer + 2
            )?;
        }
        if let Some(kmem) = self.kern_mem {
            writeln!(
                f,
                "{:0w$}KernelMem[size={} KiB],",
                "",
                kmem / 1024,
                w = layer + 2
            )?;
        }
        for m in &self.phys_mems {
            writeln!(
                f,
                "{:0w$}PhysMem[addr={:#x}, size={:#x} KiB],",
                "",
                m.phys(),
                m.size() / 1024,
                w = layer + 2
            )?;
        }
        for s in &self.services {
            writeln!(
                f,
                "{:0w$}Service[lname={}, gname={}],",
                "",
                s.local_name,
                s.global_name,
                w = layer + 2
            )?;
        }
        for s in &self.sessions {
            writeln!(
                f,
                "{:0w$}Session[lname={}, gname={}, arg={}],",
                "",
                s.local_name,
                s.serv,
                s.arg,
                w = layer + 2
            )?;
        }
        for pe in &self.pes {
            writeln!(
                f,
                "{:0w$}PE[type={}, count={}, optional={}],",
                "",
                pe.ty,
                pe.count.get(),
                pe.optional,
                w = layer + 2
            )?;
        }
        write!(f, "{:0w$}]", "", w = layer)
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "Config [")?;
        for d in &self.doms {
            writeln!(f, "{:?}", d)?;
        }
        write!(f, "]")
    }
}

impl fmt::Debug for Domain {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "  Domain [")?;
        for a in &self.apps {
            writeln!(f, "{:?}", a)?;
        }
        write!(f, "  ]")
    }
}

impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.print_rec(f, 4)
    }
}
