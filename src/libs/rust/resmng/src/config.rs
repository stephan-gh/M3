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
use m3::cell::Cell;
use m3::col::{BTreeMap, BTreeSet, String, Vec};
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif;
use m3::rc::Rc;
use m3::tcu::Label;

use crate::parser;
use crate::pes;

#[derive(Default)]
pub struct DualName {
    pub(crate) local: String,
    pub(crate) global: String,
}

impl DualName {
    pub fn local(&self) -> &String {
        &self.local
    }

    pub fn global(&self) -> &String {
        &self.global
    }
}

impl fmt::Debug for DualName {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "lname='{}', gname='{}'", self.local, self.global)
    }
}

pub struct PhysMemDesc {
    // TODO add memory id
    phys: goff,
    size: goff,
    perm: kif::Perm,
}

impl PhysMemDesc {
    pub(crate) fn new(phys: goff, size: goff, perm: kif::Perm) -> Self {
        Self { phys, size, perm }
    }

    pub fn phys(&self) -> goff {
        self.phys
    }

    pub fn size(&self) -> goff {
        self.size
    }

    pub fn perm(&self) -> kif::Perm {
        self.perm
    }
}

#[derive(Default)]
pub struct MountDesc {
    fs: String,
    path: String,
}

impl MountDesc {
    pub(crate) fn new(fs: String, path: String) -> Self {
        Self { fs, path }
    }

    pub fn fs(&self) -> &String {
        &self.fs
    }

    pub fn path(&self) -> &String {
        &self.path
    }
}

#[derive(Default)]
pub struct ServiceDesc {
    name: DualName,
    used: Cell<bool>,
}

impl ServiceDesc {
    pub(crate) fn new(name: DualName) -> Self {
        Self {
            name,
            used: Cell::new(false),
        }
    }

    pub fn name(&self) -> &DualName {
        &self.name
    }

    pub fn is_used(&self) -> bool {
        self.used.get()
    }

    pub fn mark_used(&self) {
        self.used.replace(true);
    }
}

#[derive(Default)]
pub struct SessCrtDesc {
    name: String,
    count: Option<u32>,
}

impl SessCrtDesc {
    pub(crate) fn new(name: String, count: Option<u32>) -> Self {
        Self { name, count }
    }

    pub fn serv_name(&self) -> &String {
        &self.name
    }

    pub fn sess_count(&self) -> Option<u32> {
        self.count
    }
}

#[derive(Default)]
pub struct SessionDesc {
    name: DualName,
    arg: String,
    dep: bool,
    used: Cell<bool>,
}

impl SessionDesc {
    pub(crate) fn new(name: DualName, arg: String, dep: bool) -> Self {
        Self {
            name,
            arg,
            dep,
            used: Cell::new(false),
        }
    }

    pub fn is_dep(&self) -> bool {
        self.dep
    }

    pub fn name(&self) -> &DualName {
        &self.name
    }

    pub fn arg(&self) -> &String {
        &self.arg
    }

    pub fn is_used(&self) -> bool {
        self.used.get()
    }

    pub fn mark_used(&self) {
        self.used.replace(true);
    }
}

pub struct RGateDesc {
    name: DualName,
    msg_size: usize,
    slots: usize,
}

impl RGateDesc {
    pub(crate) fn new(name: DualName, msg_size: usize, slots: usize) -> Self {
        Self {
            name,
            msg_size,
            slots,
        }
    }

    pub fn name(&self) -> &DualName {
        &self.name
    }

    pub fn msg_size(&self) -> usize {
        self.msg_size
    }

    pub fn slots(&self) -> usize {
        self.slots
    }
}

pub struct SGateDesc {
    name: DualName,
    credits: u32,
    label: Label,
    used: Cell<bool>,
}

impl SGateDesc {
    pub(crate) fn new(name: DualName, credits: u32, label: Label) -> Self {
        Self {
            name,
            credits,
            label,
            used: Cell::new(false),
        }
    }

    pub fn name(&self) -> &DualName {
        &self.name
    }

    pub fn credits(&self) -> u32 {
        self.credits
    }

    pub fn label(&self) -> Label {
        self.label
    }

    pub fn is_used(&self) -> bool {
        self.used.get()
    }

    pub fn mark_used(&self) {
        self.used.replace(true);
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
        Self {
            ty,
            count: Cell::new(count),
            optional,
        }
    }

    pub fn pe_type(&self) -> &String {
        &self.ty
    }

    pub fn count(&self) -> u32 {
        self.count.get()
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
    name: DualName,
}

impl SemDesc {
    pub(crate) fn new(name: DualName) -> Self {
        SemDesc { name }
    }

    pub fn name(&self) -> &DualName {
        &self.name
    }
}

#[derive(Default)]
pub struct Domain {
    pub(crate) pseudo: bool,
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
    pub(crate) cfg_range: (usize, usize),
    pub(crate) daemon: bool,
    pub(crate) eps: Option<u32>,
    pub(crate) user_mem: Option<usize>,
    pub(crate) kern_mem: Option<usize>,
    pub(crate) domains: Vec<Domain>,
    pub(crate) mounts: Vec<MountDesc>,
    pub(crate) phys_mems: Vec<PhysMemDesc>,
    pub(crate) services: Vec<ServiceDesc>,
    pub(crate) sesscrt: Vec<SessCrtDesc>,
    pub(crate) sessions: Vec<SessionDesc>,
    pub(crate) rgates: Vec<RGateDesc>,
    pub(crate) sgates: Vec<SGateDesc>,
    pub(crate) sems: Vec<SemDesc>,
    pub(crate) pes: Vec<PEDesc>,
}

impl AppConfig {
    pub fn parse(xml: &str) -> Result<Self, Error> {
        parser::parse(xml)
    }

    pub fn new(args: Vec<String>) -> Self {
        assert!(!args.is_empty());
        Self {
            name: args[0].clone(),
            args,
            ..Default::default()
        }
    }

    pub fn cfg_range(&self) -> (usize, usize) {
        self.cfg_range
    }

    pub fn daemon(&self) -> bool {
        self.daemon
    }

    pub fn eps(&self) -> Option<u32> {
        self.eps
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

    pub fn domains(&self) -> &Vec<Domain> {
        &self.domains
    }

    pub fn mounts(&self) -> &Vec<MountDesc> {
        &self.mounts
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

    pub fn pes(&self) -> &Vec<PEDesc> {
        &self.pes
    }

    pub fn sess_creators(&self) -> &Vec<SessCrtDesc> {
        &self.sesscrt
    }

    pub fn rgates(&self) -> &Vec<RGateDesc> {
        &self.rgates
    }

    pub fn sgates(&self) -> &Vec<SGateDesc> {
        &self.sgates
    }

    pub fn get_rgate(&self, lname: &str) -> Option<&RGateDesc> {
        self.rgates.iter().find(|r| r.name().local() == lname)
    }

    pub fn get_sgate(&self, lname: &str) -> Option<&SGateDesc> {
        self.sgates.iter().find(|s| s.name().local() == lname)
    }

    pub fn get_sem(&self, lname: &str) -> Option<&SemDesc> {
        self.sems.iter().find(|s| s.name().local() == lname)
    }

    pub fn get_service(&self, lname: &str) -> Option<&ServiceDesc> {
        self.services.iter().find(|s| s.name().local() == lname)
    }

    pub fn unreg_service(&self, gname: &str) {
        let serv = self
            .services
            .iter()
            .find(|s| s.name().global() == gname)
            .unwrap();
        serv.used.replace(false);
    }

    pub fn get_session(&self, lname: &str) -> Option<(usize, &SessionDesc)> {
        self.sessions
            .iter()
            .position(|s| s.name().local() == lname)
            .map(|idx| (idx, &self.sessions[idx]))
    }

    pub fn close_session(&self, idx: usize) {
        self.sessions[idx].used.replace(false);
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

    pub fn count_apps(&self) -> usize {
        self.domains.iter().fold(0, |total, d| total + d.apps.len())
    }

    pub fn split_child_mem(&self, user_mem: &mut goff) -> goff {
        if !self.domains().is_empty() {
            let old_user_mem = *user_mem;
            let mut def_childs = 0;
            for d in self.domains() {
                for a in d.apps() {
                    if let Some(cmem) = a.user_mem() {
                        *user_mem -= cmem as goff;
                    }
                    else {
                        def_childs += 1;
                    }
                }
            }
            let per_child = *user_mem / (def_childs + 1);
            *user_mem -= per_child * def_childs;
            old_user_mem - *user_mem
        }
        else {
            0
        }
    }

    pub fn check(&self) {
        self.check_services(&BTreeSet::new());
        self.check_gates();
        self.check_pes();
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

    fn check_pes(&self) {
        for d in &self.domains {
            for a in &d.apps {
                a.check_pes();
            }
        }

        for pe in &self.pes {
            if !pe.optional {
                let available = Self::count_pes(&pe);
                if available < pe.count.get() {
                    panic!(
                        "AppConfig '{}' needs PE type '{}' {} times, but {} are available",
                        self.name(),
                        pe.ty,
                        pe.count.get(),
                        available
                    );
                }
            }
        }
    }

    fn check_services(&self, parent_set: &BTreeSet<String>) {
        let mut set = BTreeSet::new();
        for d in &self.domains {
            for a in &d.apps {
                for serv in a.services() {
                    if set.contains(serv.name().global()) {
                        panic!(
                            "config '{}': service '{}' does already exist",
                            a.name(),
                            serv.name().global()
                        );
                    }
                    set.insert(serv.name().global().clone());
                }
            }
        }

        let mut subset = set.clone();
        for s in parent_set.iter() {
            if !subset.contains(s) {
                subset.insert(s.clone());
            }
        }
        for d in &self.domains {
            for a in &d.apps {
                a.check_services(&subset);
            }
        }

        for sess in self.sessions() {
            if !set.contains(sess.name().global()) && !parent_set.contains(sess.name().global()) {
                panic!(
                    "config '{}': service '{}' does not exist",
                    self.name(),
                    sess.name().global()
                );
            }
        }
    }

    fn check_gates(&self) {
        let mut map = BTreeMap::new();
        for d in &self.domains {
            for a in &d.apps {
                for rgate in a.rgates() {
                    if map.contains_key(rgate.name().global()) {
                        panic!(
                            "config '{}': rgate '{}' does already exist",
                            a.name(),
                            rgate.name().global()
                        );
                    }
                    map.insert(rgate.name().global().clone(), rgate.slots());
                }
            }
        }

        for d in &self.domains {
            for a in &d.apps {
                a.check_gates();

                for sgate in a.sgates() {
                    match map.get_mut(sgate.name().global()) {
                        Some(s) => {
                            if *s == 0 {
                                panic!(
                                    "config '{}': not enough slots in rgate '{}'",
                                    a.name(),
                                    sgate.name().global()
                                );
                            }
                            *s -= 1;
                        },
                        None => panic!(
                            "config '{}': rgate '{}' does not exist",
                            a.name(),
                            sgate.name().global()
                        ),
                    }
                }
            }
        }
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
        if let Some(eps) = self.eps {
            writeln!(f, "{:0w$}Endpoints[count={}],", "", eps, w = layer + 2)?;
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
                "{:0w$}PhysMem[addr={:#x}, size={:#x} KiB, perm={:?}],",
                "",
                m.phys(),
                m.size() / 1024,
                m.perm(),
                w = layer + 2
            )?;
        }
        for s in &self.services {
            writeln!(f, "{:0w$}Service[{:?}],", "", s.name, w = layer + 2)?;
        }
        for s in &self.sesscrt {
            writeln!(
                f,
                "{:0w$}SessCreator[service='{}', count={:?}],",
                "",
                s.serv_name(),
                s.sess_count(),
                w = layer + 2
            )?;
        }
        for s in &self.sessions {
            writeln!(
                f,
                "{:0w$}Session[{:?}, arg='{}', dep={}],",
                "",
                s.name,
                s.arg,
                s.dep,
                w = layer + 2
            )?;
        }
        for r in &self.rgates {
            writeln!(
                f,
                "{:0w$}RGate[{:?}, msgsize='{}', slots={}],",
                "",
                r.name,
                r.msg_size,
                r.slots,
                w = layer + 2
            )?;
        }
        for s in &self.sgates {
            writeln!(
                f,
                "{:0w$}SGate[{:?}, credits='{}', label={:#x}],",
                "",
                s.name,
                s.credits,
                s.label,
                w = layer + 2
            )?;
        }
        for m in &self.mounts {
            writeln!(
                f,
                "{:0w$}Mount[fs='{}', path='{}'],",
                "",
                m.fs,
                m.path,
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
        for d in &self.domains {
            let mut sub_layer = layer;
            if !d.pseudo {
                writeln!(f, "{:0w$}Domain [", "", w = layer + 2)?;
                sub_layer += 2;
            }
            for a in &d.apps {
                a.print_rec(f, sub_layer + 2)?;
            }
            if !d.pseudo {
                writeln!(f, "{:0w$}]", "", w = layer + 2)?;
            }
        }
        writeln!(f, "{:0w$}]", "", w = layer)
    }
}

impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "Config [")?;
        self.print_rec(f, 2)?;
        writeln!(f, "]")
    }
}
