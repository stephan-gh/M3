pub mod ioctl;
pub mod mmap;

use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

use crate::cell::{LazyStaticCell, LazyStaticRefCell};
use crate::cfg;
use crate::env;
use crate::kif::Perm;
use crate::tcu;
use crate::time::TimeDuration;

static TCU_DEV: LazyStaticRefCell<File> = LazyStaticRefCell::default();
static TCU_EPOLL_FD: LazyStaticCell<libc::c_int> = LazyStaticCell::default();

pub fn tcu_fd() -> libc::c_int {
    TCU_DEV.borrow().as_raw_fd()
}

pub fn init_fd() {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_SYNC)
        .open("/dev/tcu")
        .expect("Unable to open /dev/tcu");
    TCU_DEV.set(file);

    // size argument is ignored since 2.6.8, but needs to be non-zero
    let epoll_fd = unsafe { libc::epoll_create(1) };
    assert!(epoll_fd != -1);

    let mut ev = libc::epoll_event {
        r#u64: tcu_fd() as u64,
        events: libc::EPOLLIN as u32,
    };
    assert_eq!(
        unsafe { libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, tcu_fd(), &mut ev) },
        0
    );

    TCU_EPOLL_FD.set(epoll_fd);
}

pub fn wait_msg(timeout: Option<TimeDuration>) {
    let timeout = match timeout {
        Some(duration) => duration.as_millis() as libc::c_int,
        None => -1,
    };

    let mut ev = libc::epoll_event {
        r#u64: 0,
        events: 0,
    };
    unsafe {
        libc::epoll_wait(TCU_EPOLL_FD.get(), &mut ev, 1, timeout);
    }
}

pub fn init_env() {
    mmap::mmap_tcu(
        tcu_fd(),
        cfg::ENV_START,
        cfg::ENV_SIZE,
        mmap::MemType::Environment,
        Perm::RW,
    )
    .expect("Unable to map environment");
}

pub fn init() {
    init_fd();

    init_env();

    mmap::mmap_tcu(
        tcu_fd(),
        tcu::MMIO_ADDR,
        tcu::MMIO_SIZE,
        mmap::MemType::TCU,
        Perm::RW,
    )
    .expect("Unable to map TCU MMIO region");

    #[cfg(not(any(feature = "hw22", feature = "hw23")))]
    mmap::mmap_tcu(
        tcu_fd(),
        tcu::MMIO_EPS_ADDR,
        tcu::TCU::endpoints_size(),
        mmap::MemType::TCUEPs,
        Perm::R,
    )
    .expect("Unable to map TCU-EPs MMIO region");

    let (rbuf_virt_addr, rbuf_size) = env::boot().tile_desc().rbuf_std_space();
    mmap::mmap_tcu(
        tcu_fd(),
        rbuf_virt_addr,
        rbuf_size,
        mmap::MemType::StdRecvBuf,
        Perm::R,
    )
    .expect("Unable to map standard receive buffer");
}
