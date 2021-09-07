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

use m3::cell::StaticCell;
use m3::col::Treap;
use m3::errors::{Code, Error};
use m3::tcu;

use crate::childs::Id;

static CHILD_EVENTS: StaticCell<Treap<Id, Option<u64>>> = StaticCell::new(Treap::new());

pub fn alloc_event() -> thread::Event {
    static NEXT_ID: StaticCell<u64> = StaticCell::new(0);
    NEXT_ID.set(*NEXT_ID + 1);
    0x8000_0000_0000_0000 | *NEXT_ID
}

pub fn wait_for_async(child: Id, event: thread::Event) -> Result<&'static tcu::Message, Error> {
    // remember that the child waits for this event in case we remove it in the meantime
    CHILD_EVENTS.get_mut().set(child, Some(event));

    thread::ThreadManager::get().wait_for(event);

    // waiting done, remove it again (this potentially adds an entry into the Treap again)
    CHILD_EVENTS.get_mut().set(child, None);

    // fetch message for caller
    thread::ThreadManager::get()
        .fetch_msg()
        .ok_or_else(|| Error::new(Code::RecvGone))
}

pub fn remove_child(child: Id) {
    // if the child is currently waiting for an event, let this fail by delivering a None message
    if let Some(Some(event)) = CHILD_EVENTS.get_mut().remove(&child) {
        thread::ThreadManager::get().notify(event, None);
    }
}
