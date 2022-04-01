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
use m3::session::ServerSession;

use crate::chan::Channel;
use crate::meta::Meta;
use crate::pipe::Pipe;

pub enum SessionData {
    Meta(Meta),
    Pipe(Pipe),
    Chan(Channel),
}

pub struct PipesSession {
    crt: usize,
    sess: ServerSession,
    data: SessionData,
}

impl PipesSession {
    pub fn new(crt: usize, sess: ServerSession, data: SessionData) -> Self {
        PipesSession { crt, sess, data }
    }

    pub fn creator(&self) -> usize {
        self.crt
    }

    pub fn sel(&self) -> Selector {
        self.sess.sel()
    }

    pub fn data(&self) -> &SessionData {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut SessionData {
        &mut self.data
    }
}
