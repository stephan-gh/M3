use log::{debug, trace};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::Write;

use crate::error::Error;
use crate::symbols;

const STACK_SIZE: u64 = 0x4000;

#[derive(Default)]
struct PE {
    cur_tid: ThreadId,
    stacks: BTreeMap<ThreadId, Thread>,
}

#[derive(Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq)]
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
    fn binary_switch(&mut self, sym: &symbols::Symbol, time: u64) {
        let mut new_tid = ThreadId {
            bin: sym.bin.clone(),
            stack: !0,
        };

        match self.stacks.range(..=&new_tid).nth_back(0) {
            Some((tid, _)) if tid.bin == sym.bin => {
                // we know the stack, switch to it
                self.cur_tid = tid.clone();
                debug!("{}: switched to {}", time, self.cur_tid);
            },
            _ => {
                new_tid.stack = 0;
                // create new stack
                self.cur_tid = new_tid.clone();
                self.stacks.insert(self.cur_tid.clone(), Thread::default());
                debug!("{}: new binary {}", time, self.cur_tid);
            },
        }
    }

    fn found_stack(&mut self, sym: &symbols::Symbol, tid: u64, time: u64) {
        let old = self.stacks.remove(&self.cur_tid).unwrap();
        self.cur_tid = ThreadId {
            bin: sym.bin.clone(),
            stack: tid - STACK_SIZE,
        };
        self.stacks.insert(self.cur_tid.clone(), old);
        debug!("{}: found stack of {}", time, self.cur_tid);
    }

    fn thread_switch(&mut self, sym: &symbols::Symbol, stack: u64, time: u64) {
        // remember switch time (see below)
        self.stacks.get_mut(&self.cur_tid).unwrap().switched = time;

        // try to find the thread with new stack
        let mut new_tid = ThreadId {
            bin: sym.bin.clone(),
            stack,
        };
        match self.stacks.range(..=&new_tid).nth_back(0) {
            Some((tid, _))
                if tid.bin == sym.bin && stack >= tid.stack && stack < tid.stack + STACK_SIZE =>
            {
                // we know the stack, switch to it
                self.cur_tid = tid.clone();
                debug!("{}: switched back to {}", time, self.cur_tid);
            }
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

    fn call(&mut self, sym: &symbols::Symbol, time: u64) {
        trace!("{}: {:w$} CALL -> {}", time, "", sym.name, w = self.depth());
        self.stack.push(Call {
            func: sym.name.clone(),
            time,
        });
    }

    fn ret(&mut self, sym: &symbols::Symbol, time: u64, tid: &ThreadId) -> Call {
        let mut last = self.stack.pop().unwrap();
        // unwind the stack until we find the function on the stack that matches the current symbol
        loop {
            match self.stack.last() {
                Some(f) if f.func == sym.name => {
                    trace!("{}: {:w$} RET  -> {}", time, "", sym.name, w = self.depth());
                    return last;
                },
                Some(_) => last = self.stack.pop().unwrap(),
                None => panic!("{}: {}: expected return to {}", time, tid, sym.name),
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

pub fn generate(isa: &crate::ISA, syms: &BTreeMap<usize, symbols::Symbol>) -> Result<(), Error> {
    let mut pes: HashMap<usize, PE> = HashMap::new();

    crate::with_stdin_lines(syms, |syms, writer, line| {
        if let Some((time, pe, addr)) = get_func_addr(line) {
            if let Some(sym) = symbols::resolve(syms, addr) {
                // detect PEs
                if pes.get(&pe).is_none() {
                    let mut new_pe = PE::default();
                    new_pe
                        .stacks
                        .insert(new_pe.cur_tid.clone(), Thread::default());
                    pes.insert(pe, new_pe);
                }
                let cur_pe = pes.get_mut(&pe).unwrap();

                // detect binary changes (e.g., dtumux to app)
                if sym.bin != cur_pe.cur_tid.bin {
                    cur_pe.binary_switch(&sym, time);
                }

                // detect the stack pointer
                if cur_pe.cur_tid.stack == 0 && instr_is_sp_assign(isa, line) {
                    if let Some(pos) = line.find("D=") {
                        let tid = u64::from_str_radix(&line[(pos + 4)..(pos + 20)], 16)?;
                        cur_pe.found_stack(&sym, tid, time);
                    }
                }

                // detect thread switches
                if sym.name == "thread_resume" && instr_is_sp_init(isa, line) {
                    if let Some(pos) = line.find("D=") {
                        let tid = u64::from_str_radix(&line[(pos + 4)..(pos + 20)], 16)?;
                        cur_pe.thread_switch(&sym, tid, time);
                    }
                }

                let cur_thread = cur_pe.stacks.get_mut(&cur_pe.cur_tid).unwrap();

                // function changed?
                if sym.addr != cur_thread.last_func {
                    // it's a call when we jumped to the beginning of a function
                    if addr == sym.addr {
                        cur_thread.call(&sym, time);
                    }
                    // otherwise it's a return
                    else {
                        if sym.name != "thread_resume" && cur_thread.stack.is_empty() {
                            panic!("{}: return with empty stack", time);
                        }

                        if !cur_thread.stack.is_empty() {
                            // generate stack
                            let mut stack: String = format!("PE{}", pe);
                            stack.push_str(";");
                            stack.push_str(&format!("{}", cur_pe.cur_tid));
                            for f in cur_thread.stack.iter() {
                                stack.push_str(";");
                                stack.push_str(&f.func);
                            }

                            // print flamegraph line
                            let last = cur_thread.ret(&sym, time, &cur_pe.cur_tid);
                            writeln!(writer, "{} {}", stack, (time - last.time) / 1000)?;
                        }
                    }
                }

                cur_thread.last_func = sym.addr;
            }
        }

        Ok(())
    })
}
