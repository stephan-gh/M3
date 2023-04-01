use crate::errors::{Code, Error};
use crate::cell::LazyStaticRefCell;

use std::os::unix::io::AsRawFd;
use std::fs::File;

use libc;

static MEM_DEV: LazyStaticRefCell<File> = LazyStaticRefCell::default();

pub fn mmap(addr: usize, size: usize) -> Result<(), Error> {
    let base = unsafe {
        libc::mmap(
            addr as *mut libc::c_void,
            size,
            libc::PROT_READ,
            libc::MAP_SHARED,
            MEM_DEV.borrow().as_raw_fd(),
            0,
        )
    };
    if base as usize == addr {
        Ok(())
    } else {
        Err(Error::new(Code::InvArgs))
    }
}

pub fn munmap(addr: usize, size: usize) {
    unsafe {
        libc::munmap(addr as *mut libc::c_void, size);
    }
}

pub fn init() {
    MEM_DEV.set(std::fs::File::open("/dev/mem").unwrap());
}
