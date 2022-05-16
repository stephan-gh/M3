/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

use core::cmp;

use crate::col::DList;
use crate::net::{event, Endpoint, IpAddr, NetEvent, Port};

struct Item {
    event: NetEvent,
    pos: usize,
}

impl Item {
    fn data(&self) -> &[u8] {
        &self.msg().data[self.pos..self.size()]
    }

    fn size(&self) -> usize {
        self.msg().size as usize
    }

    fn addr(&self) -> IpAddr {
        IpAddr(self.msg().addr as u32)
    }

    fn port(&self) -> Port {
        self.msg().port as Port
    }

    fn msg(&self) -> &event::DataMessage {
        self.event.msg::<event::DataMessage>()
    }
}

#[derive(Default)]
pub struct DataQueue {
    items: DList<Item>,
}

impl DataQueue {
    pub fn append(&mut self, event: NetEvent, pos: usize) {
        self.items.push_back(Item { event, pos });
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn has_data(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn next_data<F, R>(&mut self, len: usize, consume: &mut F) -> Option<(usize, R)>
    where
        F: FnMut(&[u8], Endpoint) -> (usize, R),
    {
        if let Some(first) = self.items.front_mut() {
            let data = first.data();
            let amount = cmp::min(len, data.len());
            let ep = Endpoint::new(first.addr(), first.port());
            let (amount, res) = consume(&data[0..amount], ep);
            if amount >= data.len() {
                self.items.pop_front();
            }
            else if amount == 0 {
                return None;
            }
            else {
                first.pos += amount;
            }
            Some((amount, res))
        }
        else {
            None
        }
    }
}
