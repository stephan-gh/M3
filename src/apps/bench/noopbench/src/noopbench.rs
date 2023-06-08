#![no_std]

use m3::{
    build_vmsg,
    col::String,
    errors::{Code, Error},
    io::{Read, Write},
    kif::{self, Perm},
    mem::{MsgBuf, VirtAddr},
    println,
    serialize::M3Deserializer,
    tcu::{self, EpId},
    tiles::Activity,
    time::{CycleDuration, CycleInstant, Duration, Profiler, Results, Runner},
    vfs::{FileMode, FileRef, GenericFile, OpenFlags, VFS},
};

fn wait_for_rpl(rep: EpId, rcv_buf: VirtAddr) -> Result<(), Error> {
    loop {
        if let Some(off) = tcu::TCU::fetch_msg(rep) {
            let msg = tcu::TCU::offset_to_msg(rcv_buf, off);
            let mut de = M3Deserializer::new(msg.as_words());
            let res: Code = de.pop()?;
            tcu::TCU::ack_msg(rep, off)?;
            return Result::from(res);
        }
    }
}

fn noop_syscall(rbuf: VirtAddr) {
    let mut msg = MsgBuf::borrow_def();
    build_vmsg!(msg, kif::syscalls::Operation::Noop, kif::syscalls::Noop {});
    tcu::TCU::send(
        tcu::FIRST_USER_EP + tcu::SYSC_SEP_OFF,
        &msg,
        0,
        tcu::FIRST_USER_EP + tcu::SYSC_REP_OFF,
    )
    .unwrap();
    wait_for_rpl(tcu::FIRST_USER_EP + tcu::SYSC_REP_OFF, rbuf).unwrap();
}

#[inline(never)]
fn bench_custom_noop_syscall(profiler: &Profiler) -> Results<CycleDuration> {
    let (rbuf, _) = Activity::own().tile_desc().rbuf_std_space();
    profiler.run::<CycleInstant, _>(|| {
        noop_syscall(rbuf);
    })
}

#[inline(never)]
fn bench_m3_noop_syscall(profiler: &Profiler) -> Results<CycleDuration> {
    profiler.run::<CycleInstant, _>(|| {
        m3::syscalls::noop().unwrap();
    })
}

#[inline(never)]
fn bench_tlb_insert(profiler: &Profiler) -> Results<CycleDuration> {
    let sample_addr = profiler as *const Profiler as usize;
    profiler.run::<CycleInstant, _>(|| {
        tcu::TCU::handle_xlate_fault(sample_addr, Perm::R);
    })
}

#[inline(never)]
fn bench_os_call(profiler: &Profiler) -> Results<CycleDuration> {
    profiler.run::<CycleInstant, _>(|| {
        m3::tmif::noop().unwrap();
    })
}

const READ_STR_LEN: usize = 1024 * 1024;
const WRITE_STR_LEN: usize = 8 * 1024;

#[inline(never)]
fn bench_m3fs_read(profiler: &Profiler) -> Results<CycleDuration> {
    let mut file = VFS::open("/new-file.txt", OpenFlags::CREATE | OpenFlags::RW).unwrap();
    let content: String = (0..READ_STR_LEN).map(|_| "a").collect();
    write!(file, "{}", content).unwrap();

    let res = profiler.run::<CycleInstant, _>(|| {
        let _content = file.read_to_string().unwrap();
    });

    VFS::unlink("/new-file.txt").unwrap();
    res
}

struct WriteBenchmark {
    file: FileRef<GenericFile>,
    content: String,
}

impl WriteBenchmark {
    fn new() -> WriteBenchmark {
        WriteBenchmark {
            file: VFS::open("/new-file.txt", OpenFlags::CREATE | OpenFlags::W).unwrap(),
            content: (0..WRITE_STR_LEN).map(|_| "a").collect(),
        }
    }
}

impl Drop for WriteBenchmark {
    fn drop(&mut self) {
        VFS::unlink("/new-file.txt").unwrap();
    }
}

impl Runner for WriteBenchmark {
    fn run(&mut self) {
        self.file.write_all(self.content.as_bytes()).unwrap();
    }

    fn post(&mut self) {
        self.file.borrow().truncate(0).unwrap();
    }
}

#[inline(never)]
fn bench_m3fs_write(profiler: &Profiler) -> Results<CycleDuration> {
    profiler.runner::<CycleInstant, _>(&mut WriteBenchmark::new())
}

#[inline(never)]
fn bench_m3fs_meta(profiler: &Profiler) -> Results<CycleDuration> {
    profiler.run::<CycleInstant, _>(|| {
        VFS::mkdir("/new-dir", FileMode::from_bits(0o755).unwrap()).unwrap();
        let _ = VFS::stat("/new-dir").unwrap();
        {
            let _ = VFS::open("/new-dir/new-file", OpenFlags::CREATE).unwrap();
        }
        {
            let mut file = VFS::open("/new-dir/new-file", OpenFlags::W).unwrap();
            write!(file, "test").unwrap();
        }
        {
            let mut file = VFS::open("/new-dir/new-file", OpenFlags::R).unwrap();
            let _ = file.read_to_string().unwrap();
            let _ = VFS::stat("/new-dir/new-file").unwrap();
        }

        VFS::link("/new-dir/new-file", "/new-link").unwrap();
        VFS::rename("/new-link", "/new-blink").unwrap();
        let _ = VFS::stat("/new-blink");
        VFS::unlink("/new-blink").unwrap();
        VFS::unlink("/new-dir/new-file").unwrap();
        VFS::rmdir("/new-dir").unwrap();
    })
}

fn print_summary<T: Duration + Clone>(name: &str, res: &Results<T>) {
    println!("{}: {}", name, res);
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    VFS::mount("/", "m3fs", "m3fs").unwrap();
    let profiler = Profiler::default().warmup(10).repeats(100);

    let meta = bench_m3fs_meta(&profiler);
    print_summary("m3fs meta", &meta);
    let cnoop = bench_custom_noop_syscall(&profiler);
    print_summary("custom noop", &cnoop);
    let m3noop = bench_m3_noop_syscall(&profiler);
    print_summary("m3 noop", &m3noop);
    let oscall = bench_os_call(&profiler);
    print_summary("oscall", &oscall);
    let tlb = bench_tlb_insert(&profiler);
    print_summary("tlb insert", &tlb);
    let read = bench_m3fs_read(&profiler);
    print_summary("m3fs read", &read);
    let write = bench_m3fs_write(&profiler);
    print_summary("m3fs write", &write);
    Ok(())
}
