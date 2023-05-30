use libc;

use crate::errors::{Code, Error};

pub fn mmap(addr: usize, size: usize) -> Result<(), Error> {
    let base = unsafe {
        libc::mmap(
            addr as *mut libc::c_void,
            size,
            libc::PROT_READ,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        )
    };
    if base as usize == addr {
        Ok(())
    }
    else {
        Err(Error::new(Code::InvArgs))
    }
}

pub fn munmap(addr: usize, size: usize) {
    unsafe {
        libc::munmap(addr as *mut libc::c_void, size);
    }
}
