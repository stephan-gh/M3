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
use m3::cfg;
use m3::col::ToString;
use m3::errors::Error;
use m3::goff;
use m3::math;
use m3::pes::{PE, VPE};
use m3::rc::Rc;

use resmng::{config, memory, sems};

#[derive(Default)]
pub struct Arguments {
    pub share_pe: bool,
    pub share_kmem: bool,
}

pub struct Config {
    root: config::AppConfig,
}

impl Config {
    pub fn parse(xml: &str, restrict: bool) -> Result<Self, Error> {
        Ok(Self {
            root: config::AppConfig::parse(xml, restrict)?,
        })
    }

    pub fn root(&self) -> &config::AppConfig {
        &self.root
    }

    pub fn parse_args(&self) -> Arguments {
        let mut args = Arguments::default();
        // parse our own arguments
        for arg in self.root.args() {
            if arg == "sharekmem" {
                args.share_kmem = true;
            }
            else if arg == "sharepe" {
                args.share_pe = true;
            }
            else if arg.starts_with("sem=") {
                sems::get()
                    .add_sem(arg[4..].to_string())
                    .expect("Unable to add semaphore");
            }
        }
        args
    }

    pub fn split_mem(&self, mems: &memory::MemModCon) -> (usize, goff) {
        let mut total_umem = mems.capacity();
        let mut total_kmem = VPE::cur()
            .kmem()
            .quota()
            .expect("Unable to determine own quota");

        let mut total_kparties = self.root.count_apps() + 1;
        let mut total_mparties = total_kparties;
        for d in self.root.domains() {
            for a in d.apps() {
                if let Some(kmem) = a.kernel_mem() {
                    total_kmem -= kmem;
                    total_kparties -= 1;
                }

                if let Some(amem) = a.user_mem() {
                    total_umem -= amem as goff;
                    total_mparties -= 1;
                }
            }
        }

        let def_kmem = total_kmem / total_kparties;
        let def_umem = math::round_dn(total_umem / total_mparties as goff, cfg::PAGE_SIZE as goff);
        (def_kmem, def_umem)
    }

    pub fn split_eps(pe: &Rc<PE>, d: &config::Domain) -> Result<u32, Error> {
        let mut total_eps = pe.quota()?;
        let mut total_parties = d.apps().len();
        for cfg in d.apps() {
            if let Some(eps) = cfg.eps() {
                total_eps -= eps;
                total_parties -= 1;
            }
        }

        Ok(total_eps.checked_div(total_parties as u32).unwrap_or(0))
    }

    pub fn check(&self) {
        self.root.check();
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self.root)
    }
}
