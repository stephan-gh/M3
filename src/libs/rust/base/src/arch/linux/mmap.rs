use libc;

use num_enum::IntoPrimitive;

use crate::errors::{Code, Error};

#[derive(Copy, Clone, Debug, Eq, PartialEq, IntoPrimitive)]
#[repr(usize)]
pub enum MemType {
    TCU,
    Environment,
    StdRecvBuf,
}

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

pub fn mmap_tcu(fd: libc::c_int, addr: usize, size: usize, ty: MemType) -> Result<(), Error> {
    let base = unsafe {
        libc::mmap(
            addr as *mut libc::c_void,
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED | libc::MAP_FIXED | libc::MAP_SYNC,
            fd,
            (ty as libc::off_t) << 12,
        )
    };
    match base {
        x if x as usize == addr => Ok(()),
        _ => Err(Error::new(Code::Unspecified)),
    }
}

pub fn munmap(addr: usize, size: usize) {
    unsafe {
        libc::munmap(addr as *mut libc::c_void, size);
    }
}
