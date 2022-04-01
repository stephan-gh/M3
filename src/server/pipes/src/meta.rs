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

use m3::cap::Selector;
use m3::col::Vec;
use m3::com::RecvGate;
use m3::errors::Error;
use m3::server::SessId;

use crate::pipe::Pipe;

#[derive(Default)]
pub struct Meta {
    pipes: Vec<SessId>,
}

impl Meta {
    pub fn create_pipe(
        &mut self,
        sel: Selector,
        sid: SessId,
        mem_size: usize,
        rgate: &RecvGate,
    ) -> Result<Pipe, Error> {
        self.pipes.push(sid);
        Pipe::new(sel, sid, mem_size, rgate)
    }

    pub fn close(&mut self, sids: &mut Vec<SessId>) -> Result<(), Error> {
        sids.extend_from_slice(&self.pipes);
        Ok(())
    }
}
