use log::{debug, trace};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::{StdoutLock, Write};

use crate::error::Error;
use crate::symbols;

const STACK_SIZE: u64 = 0x4000;

struct PE {
    bins: BTreeMap<String, Binary>,
    last_bin: String,
    last_isr_exit: bool,
}

struct Binary {
    name: String,
    stacks: BTreeMap<ThreadId, Thread>,
    cur_tid: ThreadId,
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
struct ThreadId {
    bin: String,
    stack: u64,
}

#[derive(Default)]
struct Thread {
    stack: Vec<Call>,
    switched: u64,
    last_func: usize,
}

#[derive(Debug)]
struct Call {
    func: String,
    time: u64,
}

impl ThreadId {
    fn new(bin: &str) -> Self {
        Self::new_with_stack(bin, 0)
    }

    fn new_with_stack(bin: &str, stack: u64) -> Self {
        ThreadId {
            bin: bin.to_string(),
            stack,
        }
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{} [tid={:#x}]", self.bin, self.stack)
    }
}

fn get_func_addr(line: &str) -> Option<(u64, usize, usize)> {
    // get the first parts:
    // 7802000: pe00.cpu T0 : 0x226f3a @ heap_init+26    : mov rcx, DS:[rip + 0x295a7]
    // ^------^ ^------^ ^^ ^ ^------^ ^---------------------------------------------^
    let mut parts = line.splitn(6, ' ');
    let time = parts.next()?;
    let cpu = parts.next()?;
    if !cpu.ends_with(".cpu") {
        return None;
    }

    let addr = parts.nth(2)?;
    let mut addr_parts = addr.splitn(2, '.');
    let addr_int = usize::from_str_radix(&addr_parts.next()?[2..], 16).ok()?;
    let time_int = time[..time.len() - 1].parse::<u64>().ok()?;
    let cpu_int = cpu[2..4].parse::<usize>().ok()?;
    Some((time_int, cpu_int, addr_int))
}

impl PE {
    fn new(bin: Binary) -> Self {
        let mut bins = BTreeMap::new();
        let name = bin.name.clone();
        bins.insert(name.clone(), bin);
        PE {
            bins,
            last_bin: name,
            last_isr_exit: false,
        }
    }

    fn binary_switch(&mut self, sym: &symbols::Symbol, time: u64) {
        if self.bins.get(&sym.bin).is_none() {
            debug!("{}: new binary {}", time, sym.bin);
            self.bins.insert(sym.bin.clone(), Binary::new(&sym.bin));
        }
        else {
            debug!("{}: switched to {}", time, sym.bin);
        }
        self.last_bin = sym.bin.clone();
    }
}

impl Binary {
    fn new(name: &str) -> Self {
        let cur_tid = ThreadId::new(name);
        let mut stacks = BTreeMap::new();
        stacks.insert(cur_tid.clone(), Thread::default());
        Binary {
            name: name.to_string(),
            stacks,
            cur_tid,
        }
    }

    fn found_stack(&mut self, tid: u64, time: u64) {
        let old = self.stacks.remove(&self.cur_tid).unwrap();
        self.cur_tid = ThreadId::new_with_stack(&self.name, tid - STACK_SIZE);
        self.stacks.insert(self.cur_tid.clone(), old);
        debug!("{}: found stack of {}", time, self.cur_tid);
    }

    fn thread_switch(&mut self, stack: u64, time: u64) {
        // remember switch time (see below)
        self.stacks.get_mut(&self.cur_tid).unwrap().switched = time;

        // try to find the thread with new stack
        let mut new_tid = ThreadId::new_with_stack(&self.name, stack);
        match self.stacks.range(..=&new_tid).nth_back(0) {
            Some((tid, _)) if stack >= tid.stack && stack < tid.stack + STACK_SIZE => {
                // we know the stack, switch to it
                self.cur_tid = tid.clone();
                debug!("{}: switched back to {}", time, self.cur_tid);
            },
            _ => {
                // create new stack
                new_tid.stack -= STACK_SIZE;
                self.cur_tid = new_tid.clone();
                self.stacks.insert(self.cur_tid.clone(), Thread::default());
                debug!("{}: new thread {}", time, self.cur_tid);
            },
        }

        // shift the start time of all calls by the time other threads ran
        let cur_thread = self.stacks.get_mut(&self.cur_tid).unwrap();
        let duration = time - cur_thread.switched;
        for f in &mut cur_thread.stack {
            f.time += duration;
        }
    }
}

impl Thread {
    fn depth(&self) -> usize {
        self.stack.len() * 2
    }

    fn call(&mut self, sym: &symbols::Symbol, time: u64, tid: &ThreadId) {
        let w = self.depth();
        trace!("{}: {} {:w$} CALL -> {}", time, tid, "", sym.name, w = w);
        self.stack.push(Call {
            func: sym.name.clone(),
            time,
        });
    }

    fn ret(&mut self, sym: &symbols::Symbol, time: u64, tid: &ThreadId) -> Option<Call> {
        if self.stack.iter().find(|s| s.func == sym.name).is_none() {
            trace!("{}: {} return to {} w/o preceeding call", time, tid, sym.name);
            return None;
        }

        // unwind the stack until we find the function on the stack that matches the current symbol
        let mut last = self.stack.pop().unwrap();
        loop {
            match self.stack.last() {
                Some(f) if f.func == sym.name => {
                    let w = self.depth();
                    trace!("{}: {} {:w$} RET  -> {}", time, tid, "", sym.name, w = w);
                    return Some(last);
                },
                _ => last = self.stack.pop().unwrap(),
            }
        }
    }
}

fn instr_is_sp_assign(isa: &crate::ISA, line: &str) -> bool {
    // find the "first" instruction that tells us the stack pointer
    match isa {
        crate::ISA::X86_64 => line.contains("subi   rsp, rsp, 0x8"),
        crate::ISA::ARM => line.contains("subi_uop   sp, sp,"),
    }
}

fn instr_is_sp_init(isa: &crate::ISA, line: &str) -> bool {
    // find the specific line in thread_resume that inits the stack pointer
    match isa {
        crate::ISA::X86_64 => line.contains("ld   rsp, DS:[rdi + 0x8]"),
        crate::ISA::ARM => line.contains("ldr2_uop   fp,sp,"),
    }
}

fn is_isr_exit(isa: &crate::ISA, line: &str) -> bool {
    match isa {
        crate::ISA::X86_64 => line.contains("IRET_PROT : wrip   , t0, t1"),
        crate::ISA::ARM => line.contains("movs   pc, lr"),
    }
}

fn handle_return(
    writer: &mut StdoutLock,
    time: u64,
    pe: usize,
    sym: &symbols::Symbol,
    thread: &mut Thread,
    tid: &ThreadId,
    unwind: bool,
) -> Result<(), Error> {
    if !thread.stack.is_empty() {
        // generate stack
        let mut stack: String = format!("PE{}", pe);
        stack.push_str(";");
        stack.push_str(&format!("{}", tid));
        for f in thread.stack.iter() {
            stack.push_str(";");
            stack.push_str(&f.func);
        }

        let last = if unwind {
            thread.ret(&sym, time, tid)
        }
        else {
            thread.stack.pop().unwrap();
            thread.stack.pop()
        };

        // print flamegraph line
        if let Some(l) = last {
            writeln!(writer, "{} {}", stack, (time - l.time) / 1000)?;
        }
    }
    Ok(())
}

pub fn generate(isa: &crate::ISA, syms: &BTreeMap<usize, symbols::Symbol>) -> Result<(), Error> {
    let mut pes: HashMap<usize, PE> = HashMap::new();

    crate::with_stdin_lines(syms, |syms, writer, line| {
        if let Some((time, pe, addr)) = get_func_addr(line) {
            if let Some(sym) = symbols::resolve(syms, addr) {
                // detect PEs
                if pes.get(&pe).is_none() {
                    pes.insert(pe, PE::new(Binary::new(&sym.name)));
                }
                let cur_pe = pes.get_mut(&pe).unwrap();

                // detect binary changes (e.g., pemux to app)
                let bin_switch = sym.bin != cur_pe.last_bin;
                let mut isr_exit = false;
                if bin_switch {
                    // detect ISR exits
                    if cur_pe.last_isr_exit {
                        let old_bin = cur_pe.bins.get_mut(&cur_pe.last_bin).unwrap();
                        let old_thread = old_bin.stacks.get_mut(&old_bin.cur_tid).unwrap();
                        handle_return(writer, time, pe, sym, old_thread, &old_bin.cur_tid, false)?;
                        isr_exit = true;
                    }
                    cur_pe.binary_switch(&sym, time);
                }

                let cur_bin = cur_pe.bins.get_mut(&sym.bin).unwrap();

                // detect the stack pointer
                if cur_bin.cur_tid.stack == 0 && instr_is_sp_assign(isa, line) {
                    if let Some(pos) = line.find("D=") {
                        let tid = u64::from_str_radix(&line[(pos + 4)..(pos + 20)], 16)?;
                        cur_bin.found_stack(tid, time);
                    }
                }

                // detect thread switches
                if sym.name == "thread_resume" && instr_is_sp_init(isa, line) {
                    if let Some(pos) = line.find("D=") {
                        let mut tid = u64::from_str_radix(&line[(pos + 4)..(pos + 20)], 16)?;
                        if *isa == crate::ISA::ARM {
                            // we get both FP and SP, but only care about SP
                            tid >>= 32;
                        }
                        cur_bin.thread_switch(tid, time);
                    }
                }

                let cur_thread = cur_bin.stacks.get_mut(&cur_bin.cur_tid).unwrap();

                // function changed?
                if !isr_exit && sym.addr != cur_thread.last_func {
                    // it's a call when we jumped to the beginning of a function
                    if addr == sym.addr {
                        cur_thread.call(&sym, time, &cur_bin.cur_tid);
                    }
                    // otherwise it's a return
                    else {
                        if sym.name != "thread_resume" && cur_thread.stack.is_empty() {
                            panic!("{}: return with empty stack", time);
                        }

                        handle_return(writer, time, pe, sym, cur_thread, &cur_bin.cur_tid, true)?;
                    }
                }

                cur_pe.last_isr_exit = is_isr_exit(isa, line);
                cur_thread.last_func = sym.addr;
            }
        }

        Ok(())
    })
}
