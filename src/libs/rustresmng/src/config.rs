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
use m3::col::{BTreeSet, String, ToString, Vec};
use m3::errors::{Code, Error};
use m3::kif;
use m3::rc::Rc;

use pes;

pub struct ServiceDesc {
    local_name: String,
    global_name: String,
    used: RefCell<bool>,
}

impl ServiceDesc {
    pub fn new(line: &str) -> Result<Self, Error> {
        let (lname, gname) = parse_names(line)?;
        Ok(ServiceDesc {
            local_name: lname,
            global_name: gname,
            used: RefCell::new(false),
        })
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

pub struct SessionDesc {
    local_name: String,
    serv: String,
    arg: String,
    usage: RefCell<Option<Selector>>,
}

impl SessionDesc {
    pub fn new(line: &str) -> Result<Self, Error> {
        let parts = line.split(':').collect::<Vec<&str>>();
        let (lname, serv, arg) = if parts.len() == 1 {
            (parts[0].to_string(), parts[0].to_string(), String::new())
        }
        else if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string(), String::new())
        }
        else if parts.len() == 3 {
            (
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            )
        }
        else {
            return Err(Error::new(Code::InvArgs));
        };

        Ok(SessionDesc {
            local_name: lname,
            serv,
            arg,
            usage: RefCell::new(None),
        })
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

pub struct PEDesc {
    ty: String,
    count: Cell<u32>,
    optional: bool,
}

impl PEDesc {
    pub fn new(line: &str) -> Result<Self, Error> {
        let (line, optional) = if line.ends_with("?") {
            (&line[0..line.len() - 1], true)
        }
        else {
            (line, false)
        };

        let parts = line.split(':').collect::<Vec<&str>>();
        let (ty, count) = if parts.len() == 1 {
            (parts[0].to_string(), 1)
        }
        else if parts.len() == 2 {
            (
                parts[0].to_string(),
                parts[1]
                    .parse::<u32>()
                    .map_err(|_| Error::new(Code::InvArgs))?,
            )
        }
        else {
            return Err(Error::new(Code::InvArgs));
        };

        Ok(PEDesc {
            ty,
            count: Cell::new(count),
            optional,
        })
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
            "fft" => desc.isa() == kif::PEISA::ACCEL_FFT,
            "rot13" => desc.isa() == kif::PEISA::ACCEL_ROT13,
            "ste" => desc.isa() == kif::PEISA::ACCEL_STE,
            "md" => desc.isa() == kif::PEISA::ACCEL_MD,
            "spmv" => desc.isa() == kif::PEISA::ACCEL_SPMV,
            "afft" => desc.isa() == kif::PEISA::ACCEL_AFFT,
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

pub struct ChildDesc {
    cfg: Rc<Config>,
    usage: RefCell<Option<Selector>>,
}

impl ChildDesc {
    pub fn new(cfg: Rc<Config>) -> Self {
        ChildDesc {
            cfg,
            usage: RefCell::new(None),
        }
    }

    pub fn local_name(&self) -> &String {
        &self.cfg.name
    }

    pub fn config(&self) -> Rc<Config> {
        self.cfg.clone()
    }

    pub fn is_used(&self) -> bool {
        self.usage.borrow().is_some()
    }

    pub fn mark_used(&self, sel: Selector) {
        self.usage.replace(Some(sel));
    }
}

pub struct SemDesc {
    local_name: String,
    global_name: String,
}

impl SemDesc {
    pub fn new(line: &str) -> Result<Self, Error> {
        let (lname, gname) = parse_names(line)?;
        Ok(SemDesc {
            local_name: lname,
            global_name: gname,
        })
    }

    pub fn global_name(&self) -> &String {
        &self.global_name
    }
}

fn parse_names(line: &str) -> Result<(String, String), Error> {
    let parts = line.split(':').collect::<Vec<&str>>();
    if parts.len() == 1 {
        Ok((parts[0].to_string(), parts[0].to_string()))
    }
    else if parts.len() == 2 {
        Ok((parts[0].to_string(), parts[1].to_string()))
    }
    else {
        Err(Error::new(Code::InvArgs))
    }
}

fn parse_size(s: &str) -> Result<usize, Error> {
    let mul = match s.chars().last() {
        Some(c) if c >= '0' && c <= '9' => 1,
        Some('k') | Some('K') => 1024,
        Some('m') | Some('M') => 1024 * 1024,
        Some('g') | Some('G') => 1024 * 1024 * 1024,
        _ => return Err(Error::new(Code::InvArgs)),
    };
    let num = match mul {
        1 => s.parse::<usize>(),
        _ => s[0..s.len() - 1].parse::<usize>(),
    }
    .map_err(|_| Error::new(Code::InvArgs))?;
    Ok(num * mul)
}

fn count_pes(pe: &PEDesc) -> u32 {
    let mut count = 0;
    for i in 0..pes::get().len() {
        if pe.matches(pes::get().get(i).desc()) {
            count += 1;
        }
    }
    count
}

fn check_pes(cfg: &Config) {
    for pe in &cfg.pes {
        if !pe.optional {
            let available = count_pes(&pe);
            if available < pe.count.get() {
                panic!(
                    "config '{}' needs PE type '{}' {} times, but {} are available",
                    cfg.name(),
                    pe.ty,
                    pe.count.get(),
                    available
                );
            }
        }
    }
}

fn collect_services(set: &mut BTreeSet<String>, cfg: &Config) {
    for serv in cfg.services() {
        if set.contains(serv.global_name()) {
            panic!("Service {} does already exist", serv.global_name());
        }
        set.insert(serv.global_name().clone());
    }
    for child in cfg.childs() {
        collect_services(set, &child.cfg);
    }
}

fn check_services(set: &BTreeSet<String>, cfg: &Config) {
    for sess in cfg.sessions() {
        if !set.contains(sess.serv_name()) {
            panic!("Service {} does not exist", sess.serv_name());
        }
    }
    for child in cfg.childs() {
        check_services(set, &child.cfg);
    }
}

pub fn check(cfgs: &[(Vec<String>, bool, Rc<Config>)]) {
    let mut services = BTreeSet::new();
    for (_, _, cfg) in cfgs {
        collect_services(&mut services, &cfg);
    }

    for (_, _, cfg) in cfgs {
        check_services(&services, &cfg);
    }

    for (_, _, cfg) in cfgs {
        check_pes(&cfg);
    }
}

pub struct Config {
    name: String,
    kmem: usize,
    restrict: bool,
    services: Vec<ServiceDesc>,
    sessions: Vec<SessionDesc>,
    childs: Vec<ChildDesc>,
    sems: Vec<SemDesc>,
    pes: Vec<PEDesc>,
}

impl Config {
    pub fn new(cmdline: &str, restrict: bool) -> Result<(Vec<String>, bool, Rc<Self>), Error> {
        Self::parse(cmdline, ' ', restrict)
    }

    fn parse(
        cmdline: &str,
        split: char,
        restrict: bool,
    ) -> Result<(Vec<String>, bool, Rc<Self>), Error> {
        let mut res = Config {
            name: String::new(),
            kmem: 0,
            restrict,
            services: Vec::new(),
            sessions: Vec::new(),
            childs: Vec::new(),
            sems: Vec::new(),
            pes: Vec::new(),
        };

        let mut args = Vec::new();
        let mut daemon = false;

        for (idx, a) in cmdline.split(split).enumerate() {
            if idx == 0 {
                res.name = a.to_string();
                args.push(a.to_string());
            }
            else if a.starts_with("serv=") {
                res.services.push(ServiceDesc::new(&a[5..])?);
            }
            else if a.starts_with("kmem=") {
                res.kmem = parse_size(&a[5..])?;
            }
            else if a.starts_with("sess=") {
                let sess = SessionDesc::new(&a[5..])?;

                // the pager is only used on gem5
                if cfg!(target_os = "none") || sess.serv_name() != "pager" {
                    res.sessions.push(sess);
                }
            }
            else if a.starts_with("child=") {
                let (_, _, cfg) = Self::parse(&a[6..], ';', restrict)?;
                res.childs.push(ChildDesc::new(cfg));
            }
            else if a.starts_with("sem=") {
                res.sems.push(SemDesc::new(&a[4..])?);
            }
            else if a.starts_with("pes=") {
                res.pes.push(PEDesc::new(&a[4..])?);
            }
            else if a == "daemon" {
                daemon = true;
            }
            else {
                args.push(a.to_string());
            }
        }

        Ok((args, daemon, Rc::new(res)))
    }

    pub fn restrict(&self) -> bool {
        self.restrict
    }

    pub fn kmem(&self) -> usize {
        self.kmem
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn services(&self) -> &Vec<ServiceDesc> {
        &self.services
    }

    pub fn sessions(&self) -> &Vec<SessionDesc> {
        &self.sessions
    }

    pub fn childs(&self) -> &Vec<ChildDesc> {
        &self.childs
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

    pub fn get_child(&self, lname: &str) -> Option<&ChildDesc> {
        self.childs.iter().find(|c| c.local_name() == lname)
    }

    pub fn remove_child(&self, sel: Selector) {
        if !self.restrict {
            return;
        }

        self.childs
            .iter()
            .find(|c| {
                if let Some(ref child) = *c.usage.borrow() {
                    *child == sel
                }
                else {
                    false
                }
            })
            .and_then(|c| c.usage.replace(None));
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
        writeln!(f, "{} [", self.name)?;
        if self.kmem != 0 {
            writeln!(f, "  kmem={}", self.kmem)?;
        }
        for s in &self.services {
            writeln!(
                f,
                "{:0w$}Service[lname={}, gname={}]",
                "",
                s.local_name,
                s.global_name,
                w = layer + 2
            )?;
        }
        for s in &self.sessions {
            writeln!(
                f,
                "{:0w$}Session[lname={}, gname={}, arg={}]",
                "",
                s.local_name,
                s.serv,
                s.arg,
                w = layer + 2
            )?;
        }
        for c in &self.childs {
            write!(f, "{:0w$}Child ", "", w = layer + 2)?;
            c.cfg.print_rec(f, layer + 2)?;
            writeln!(f)?;
        }
        for pe in &self.pes {
            writeln!(
                f,
                "{:0w$}PE[type={}, count={}]",
                "",
                pe.ty,
                pe.count.get(),
                w = layer + 2
            )?;
        }
        write!(f, "{:0w$}]", "", w = layer)
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.print_rec(f, 0)
    }
}
