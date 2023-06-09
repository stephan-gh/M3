use libc;

use crate::cfg;
use crate::kif;
use crate::mem::VirtAddr;

use super::tcu_fd;

// this is defined in linux/drivers/tcu/tcu.cc (and the right value will be printed on driver initialization during boot time)
const IOCTL_RGSTR_ACT: u64 = 0x00007101;
const IOCTL_TLB_INSRT: u64 = 0x40087102;
const IOCTL_UNREG_ACT: u64 = 0x00007103;
const IOCTL_NOOP: u64 = 0x00007104;
const IOCTL_NOOP_ARG: u64 = 0x40087105;

fn ioctl(magic_number: u64) {
    unsafe {
        let res = libc::ioctl(tcu_fd(), magic_number);
        if res != 0 {
            libc::perror(0 as *const u8);
            panic!("ioctl call {} failed with error {}", magic_number, res);
        }
    }
}

fn ioctl_write<T>(magic_number: u64, arg: T) {
    unsafe {
        let res = libc::ioctl(tcu_fd(), magic_number, &arg as *const _);
        if res != 0 {
            libc::perror(0 as *const u8);
            panic!("ioctl call {} failed with error {}", magic_number, res);
        }
    }
}

fn ioctl_plain(magic_number: u64, arg: usize) {
    unsafe {
        let res = libc::ioctl(tcu_fd(), magic_number, arg);
        if res != 0 {
            libc::perror(0 as *const u8);
            panic!("ioctl call {} failed with error {}", magic_number, res);
        }
    }
}

pub fn register_act() {
    ioctl(IOCTL_RGSTR_ACT);
}

pub fn tlb_insert_addr(virt: VirtAddr, perm: u8) {
    // touch the memory first to cause a page fault, because the TCU-TLB miss handler in the Linux
    // kernel cannot deal with the request if the page isn't mapped.
    let virt_ptr = virt.as_mut_ptr::<u8>();
    if (perm & kif::Perm::W.bits() as u8) != 0 {
        unsafe { virt_ptr.write_volatile(0) }
    }
    else {
        let _val = unsafe { virt_ptr.read_volatile() };
    }

    let arg = virt.as_local() & !cfg::PAGE_MASK | perm as usize;
    ioctl_plain(IOCTL_TLB_INSRT, arg);
}

pub fn unregister_act() {
    ioctl(IOCTL_UNREG_ACT);
}

pub fn noop() {
    ioctl(IOCTL_NOOP);
}

#[repr(C)]
struct NoopArg {
    arg1: u64,
    arg2: u64,
}

pub fn noop_arg(arg1: u64, arg2: u64) {
    let arg = NoopArg { arg1, arg2 };
    ioctl_write(IOCTL_NOOP_ARG, arg);
}
