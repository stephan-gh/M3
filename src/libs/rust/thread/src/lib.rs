/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#![no_std]

use base::boxed::Box;
use base::cell::{LazyStaticRefCell, Ref, StaticCell};
use base::cfg;
use base::col::{BoxList, Vec};
use base::impl_boxitem;
use base::libc;
use base::log;
use base::mem;
use base::tcu::{self, Message};
use base::vec;
use core::intrinsics::transmute;
use core::ptr::NonNull;

/// Logs thread switching etc.
pub const LOG_DEF: bool = false;

pub type Event = u64;

const MAX_MSG_SIZE: usize = 1024;

#[cfg(target_arch = "x86_64")]
#[derive(Default)]
#[repr(C, align(8))]
pub struct Regs {
    rbx: u64,
    rsp: u64,
    rbp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rflags: u64,
    rdi: u64,
}

#[cfg(target_arch = "arm")]
#[derive(Default)]
#[repr(C, align(4))]
pub struct Regs {
    r0: u32,
    r4: u32,
    r5: u32,
    r6: u32,
    r7: u32,
    r8: u32,
    r9: u32,
    r10: u32,
    r11: u32,
    r13: u32,
    r14: u32,
    cpsr: u32,
}

#[cfg(target_arch = "riscv64")]
#[derive(Default)]
#[repr(C, align(8))]
pub struct Regs {
    a0: u64,
    ra: u64,
    sp: u64,
    fp: u64,
    s1: u64,
    s2: u64,
    s3: u64,
    s4: u64,
    s5: u64,
    s6: u64,
    s7: u64,
    s8: u64,
    s9: u64,
    s10: u64,
    s11: u64,
}

#[cfg(target_arch = "x86_64")]
fn thread_init(thread: &mut Thread, func_addr: usize, arg: usize) {
    let top_idx = thread.stack.len() - 2;
    // put argument in rdi and function to return to on the stack
    thread.regs.rdi = arg as u64;
    thread.regs.rsp = &thread.stack[top_idx] as *const usize as u64;
    thread.stack[top_idx] = func_addr;
    thread.regs.rbp = thread.regs.rsp;
    thread.regs.rflags = 0x200; // enable interrupts
}

#[cfg(target_arch = "arm")]
fn thread_init(thread: &mut Thread, func_addr: usize, arg: usize) {
    let top_idx = thread.stack.len() - 2;
    thread.regs.r0 = arg as u32; // arg
    thread.regs.r13 = &thread.stack[top_idx] as *const usize as u32; // sp
    thread.regs.r11 = 0; // fp
    thread.regs.r14 = func_addr as u32; // lr
    thread.regs.cpsr = 0x13; // supervisor mode
}

#[cfg(target_arch = "riscv64")]
fn thread_init(thread: &mut Thread, func_addr: usize, arg: usize) {
    let top_idx = thread.stack.len() - 2;
    thread.regs.a0 = arg as u64;
    thread.regs.sp = &thread.stack[top_idx] as *const usize as u64;
    thread.regs.fp = 0;
    thread.regs.ra = func_addr as u64;
}

fn alloc_id() -> u32 {
    static NEXT_ID: StaticCell<u32> = StaticCell::new(0);
    NEXT_ID.set(NEXT_ID.get() + 1);
    NEXT_ID.get()
}

pub struct Thread {
    prev: Option<NonNull<Thread>>,
    next: Option<NonNull<Thread>>,
    id: u32,
    regs: Regs,
    stack: Vec<usize>,
    event: Event,
    has_msg: bool,
    msg: [mem::MaybeUninit<u64>; MAX_MSG_SIZE / 8],
}

impl_boxitem!(Thread);

extern "C" {
    fn thread_switch(o: *mut Regs, n: *mut Regs);
}

impl Thread {
    fn new_main() -> Box<Self> {
        Box::new(Thread {
            prev: None,
            next: None,
            id: alloc_id(),
            regs: Regs::default(),
            stack: Vec::new(),
            event: 0,
            has_msg: false,
            // safety: will only be safe to access if `has_msg` is true
            msg: unsafe { mem::MaybeUninit::uninit().assume_init() },
        })
    }

    pub fn new(func_addr: usize, arg: usize) -> Box<Self> {
        let mut thread = Box::new(Thread {
            prev: None,
            next: None,
            id: alloc_id(),
            regs: Regs::default(),
            stack: vec![0usize; cfg::STACK_SIZE / mem::size_of::<usize>()],
            event: 0,
            has_msg: false,
            // safety: will only be safe to access if `has_msg` is true
            msg: unsafe { mem::MaybeUninit::uninit().assume_init() },
        });

        log!(LOG_DEF, "Created thread {}", thread.id);

        thread_init(&mut thread, func_addr, arg);

        thread
    }

    pub fn is_main(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn fetch_msg(&mut self) -> Option<&'static tcu::Message> {
        if mem::replace(&mut self.has_msg, false) {
            // safety: has_msg is true and we trust the TCU
            unsafe {
                let head = self.msg.as_ptr() as *const tcu::Header;
                let slice = [head as usize, (*head).length()];
                Some(transmute(slice))
            }
        }
        else {
            None
        }
    }

    fn subscribe(&mut self, event: Event) {
        assert!(self.event == 0);
        self.event = event;
    }

    fn trigger_event(&mut self, event: Event) -> bool {
        if self.event == event {
            self.event = 0;
            true
        }
        else {
            false
        }
    }

    fn set_msg(&mut self, msg: &'static tcu::Message) {
        let size = msg.header.length() + mem::size_of::<tcu::Header>();
        self.has_msg = true;
        // safety: we trust the TCU
        unsafe {
            libc::memcpy(
                self.msg.as_ptr() as *mut libc::c_void,
                msg as *const tcu::Message as *const libc::c_void,
                size,
            );
        }
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        log!(LOG_DEF, "Thread {} destroyed", self.id);
    }
}

struct ThreadManager {
    current: Option<Box<Thread>>,
    ready: BoxList<Thread>,
    block: BoxList<Thread>,
    sleep: BoxList<Thread>,
}

static TMNG: LazyStaticRefCell<ThreadManager> = LazyStaticRefCell::default();

pub fn init() {
    TMNG.set(ThreadManager::new());
}

impl ThreadManager {
    fn new() -> Self {
        ThreadManager {
            current: Some(Thread::new_main()),
            ready: BoxList::new(),
            block: BoxList::new(),
            sleep: BoxList::new(),
        }
    }

    fn notify(&mut self, event: Event, msg: Option<&'static tcu::Message>) {
        let mut it = self.block.iter_mut();
        while let Some(t) = it.next() {
            if t.trigger_event(event) {
                if let Some(m) = msg {
                    t.set_msg(m);
                }
                log!(LOG_DEF, "Waking up thread {} for event {:#x}", t.id, event);
                let t = it.remove();
                self.ready.push_back(t.unwrap());
            }
        }
    }

    fn get_next(&mut self) -> Option<Box<Thread>> {
        if !self.ready.is_empty() {
            self.ready.pop_front()
        }
        else {
            self.sleep.pop_front()
        }
    }
}

pub fn cur() -> Ref<'static, Box<Thread>> {
    Ref::map(TMNG.borrow(), |tmng| tmng.current.as_ref().unwrap())
}

pub fn thread_count() -> usize {
    let tmng = TMNG.borrow();
    tmng.ready.len() + tmng.block.len() + tmng.sleep.len()
}

pub fn ready_count() -> usize {
    TMNG.borrow().ready.len()
}

pub fn blocked_count() -> usize {
    TMNG.borrow().block.len()
}

pub fn sleeping_count() -> usize {
    TMNG.borrow().sleep.len()
}

pub fn fetch_msg() -> Option<&'static tcu::Message> {
    match TMNG.borrow_mut().current {
        Some(ref mut t) => t.fetch_msg(),
        None => None,
    }
}

pub fn add_thread(func_addr: usize, arg: usize) {
    TMNG.borrow_mut()
        .sleep
        .push_back(Thread::new(func_addr, arg));
}

pub fn remove_thread() {
    TMNG.borrow_mut().sleep.pop_front().unwrap();
}

pub fn alloc_event() -> Event {
    static NEXT_EVENT: StaticCell<Event> = StaticCell::new(0);
    // if we have no other threads available, don't use events
    if sleeping_count() == 0 {
        0
    }
    // otherwise, use a unique number
    else {
        NEXT_EVENT.set(NEXT_EVENT.get() + 1);
        NEXT_EVENT.get()
    }
}

pub fn wait_for(event: Event) {
    let mut tmng = TMNG.borrow_mut();
    let next = tmng.get_next().unwrap();

    log!(
        LOG_DEF,
        "Thread {} waits for {:#x}, switching to {}",
        tmng.current.as_ref().unwrap().id,
        event,
        next.id
    );

    let mut cur = mem::replace(&mut tmng.current, Some(next)).unwrap();
    cur.subscribe(event);

    // safety: moving between two lists is fine
    unsafe {
        let old = Box::into_raw(cur);
        tmng.block.push_back(Box::from_raw(old));
        let next_ptr = &mut tmng.current.as_mut().unwrap().regs as *mut _;
        drop(tmng);

        thread_switch(&mut (*old).regs as *mut _, next_ptr);
    }
}

pub fn notify(event: Event, msg: Option<&'static Message>) {
    TMNG.borrow_mut().notify(event, msg)
}

pub fn try_yield() {
    let mut tmng = TMNG.borrow_mut();
    match tmng.ready.pop_front() {
        None => {},
        Some(next) => {
            log!(
                LOG_DEF,
                "Yielding from {} to {}",
                tmng.current.as_ref().unwrap().id,
                next.id
            );

            let cur = mem::replace(&mut tmng.current, Some(next)).unwrap();

            // safety: moving between two lists is fine
            unsafe {
                let old = Box::into_raw(cur);
                tmng.sleep.push_back(Box::from_raw(old));
                let next_ptr = &mut tmng.current.as_mut().unwrap().regs as *mut _;
                drop(tmng);

                thread_switch(&mut (*old).regs as *mut _, next_ptr);
            }
        },
    }
}

pub fn stop() {
    let mut tmng = TMNG.borrow_mut();
    if let Some(next) = tmng.get_next() {
        log!(
            LOG_DEF,
            "Stopping thread {}, switching to {}",
            tmng.current.as_ref().unwrap().id,
            next.id
        );

        let mut cur = mem::replace(&mut tmng.current, Some(next)).unwrap();

        let next_ptr = &mut tmng.current.as_mut().unwrap().regs as *mut _;
        drop(tmng);

        unsafe {
            thread_switch(&mut cur.regs as *mut _, next_ptr);
        }
    }
}
