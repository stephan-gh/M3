pub mod ioctl;
pub mod mmap;

use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

use crate::cell::LazyStaticRefCell;
use crate::cfg;
use crate::env;
use crate::kif::TileDesc;
use crate::tcu;

static TCU_DEV: LazyStaticRefCell<File> = LazyStaticRefCell::default();

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
}

pub fn init_env() {
    mmap::mmap_tcu(
        tcu_fd(),
        cfg::ENV_START,
        cfg::ENV_SIZE,
        mmap::MemType::Environment,
    )
    .expect("Unable to map environment");
}

pub fn init() {
    init_fd();

    init_env();

    mmap::mmap_tcu(tcu_fd(), tcu::MMIO_ADDR, tcu::MMIO_SIZE, mmap::MemType::TCU)
        .expect("Unable to map TCU MMIO region");

    let (rbuf_virt_addr, rbuf_size) = TileDesc::new_from(env::boot().tile_desc).rbuf_std_space();
    mmap::mmap_tcu(
        tcu_fd(),
        rbuf_virt_addr,
        rbuf_size,
        mmap::MemType::StdRecvBuf,
    )
    .expect("Unable to map standard receive buffer");
}
