/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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

use core::fmt;
use base::cfg;
use base::dtu;

bitflags! {
    pub struct MMUFlags : u64 {
        const PRESENT       = 0b0000_0001;
        const WRITE         = 0b0000_0010;
        const USER          = 0b0000_0100;
        const UNCACHED      = 0b0001_0000;
        const LARGE         = 0b1000_0000;
        const NOEXEC        = 0x8000_0000_0000_0000;
    }
}

#[no_mangle]
pub extern "C" fn to_mmu_pte(pte: dtu::PTE) -> u64 {
    let mut res = pte & !cfg::PAGE_MASK as u64;
    // translate NoC address to physical address
    res = (res & !0xFF00_0000_0000_0000) | ((res & 0xFF00_0000_0000_0000) >> 16);

    if (pte & dtu::PTEFlags::RWX.bits()) != 0 {
        res |= MMUFlags::PRESENT.bits();
    }
    if (pte & dtu::PTEFlags::W.bits()) != 0 {
        res |= MMUFlags::WRITE.bits();
    }
    if (pte & dtu::PTEFlags::I.bits()) != 0 {
        res |= MMUFlags::USER.bits();
    }
    if (pte & dtu::PTEFlags::UNCACHED.bits()) != 0 {
        res |= MMUFlags::UNCACHED.bits();
    }
    if (pte & dtu::PTEFlags::LARGE.bits()) != 0 {
        res |= MMUFlags::LARGE.bits();
    }
    if (pte & dtu::PTEFlags::X.bits()) == 0 {
        res |= MMUFlags::NOEXEC.bits();
    }
    res
}

#[no_mangle]
pub extern "C" fn to_dtu_pte(pte: u64) -> dtu::PTE {
    if pte == 0 {
        return 0;
    }

    let mut res = pte & !cfg::PAGE_MASK as u64;
    // translate physical address to NoC address
    res = (res & !0x0000_FF00_0000_0000) | ((res & 0x0000_FF00_0000_0000) << 16);

    if (pte & MMUFlags::PRESENT.bits()) != 0 {
        res |= dtu::PTEFlags::R.bits();
    }
    if (pte & MMUFlags::WRITE.bits()) != 0 {
        res |= dtu::PTEFlags::W.bits();
    }
    if (pte & MMUFlags::USER.bits()) != 0 {
        res |= dtu::PTEFlags::I.bits();
    }
    if (pte & MMUFlags::LARGE.bits()) != 0 {
        res |= dtu::PTEFlags::LARGE.bits();
    }
    if (pte & MMUFlags::NOEXEC.bits()) == 0 {
        res |= dtu::PTEFlags::X.bits();
    }
    res
}

#[no_mangle]
pub extern "C" fn noc_to_phys(noc: u64) -> u64 {
    (noc & !0xFF00000000000000) | ((noc & 0xFF00000000000000) >> 16)
}

#[no_mangle]
pub extern "C" fn get_pte_addr(mut virt: u64, level: u32) -> u64 {
    #[allow(clippy::erasing_op)]
    #[rustfmt::skip]
    const REC_MASK: u64 = ((cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 3))
                         | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 2))
                         | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 1))
                         | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 0))) as u64;

    // at first, just shift it accordingly.
    virt >>= cfg::PAGE_BITS + level as usize * cfg::LEVEL_BITS;
    virt <<= cfg::PTE_BITS;

    // now put in one PTE_REC_IDX's for each loop that we need to take
    let shift = (level + 1) as usize;
    let rem_mask = (1 << (cfg::PAGE_BITS + cfg::LEVEL_BITS * (cfg::LEVEL_CNT - shift))) - 1;
    virt |= REC_MASK & !rem_mask;

    // finally, make sure that we stay within the bounds for virtual addresses
    // this is because of recMask, that might actually have too many of those.
    virt &= (1 << (cfg::LEVEL_CNT * cfg::LEVEL_BITS + cfg::PAGE_BITS)) - 1;
    virt
}

#[no_mangle]
pub extern "C" fn get_pte_at(virt: u64, level: u32) -> u64 {
    let virt = get_pte_addr(virt, level);
    unsafe { *(virt as *const u64) }
}

fn get_pte_by_walk(virt: u64, perm: u64) -> u64 {
    for lvl in (0..4).rev() {
        let pte = to_dtu_pte(get_pte_at(virt, lvl));
        if lvl == 0 || (!(pte & 0xF) & perm) != 0 || (pte & dtu::PTEFlags::LARGE.bits()) != 0 {
            return pte;
        }
    }
    unreachable!();
}

#[no_mangle]
pub extern "C" fn get_pte(virt: u64, perm: u64) -> u64 {
    // translate to physical
    if (virt & 0xFFFF_FFFF_F000) == 0x0804_0201_0000 {
        // special case for root pt
        let mut pte: dtu::PTE;
        unsafe { asm!("mov %cr3, $0" : "=r"(pte)) };
        to_dtu_pte(pte | 0x3)
    }
    else if (virt & 0xFFF0_0000_0000) == 0x0800_0000_0000 {
        // in the PTE area, we can assume that all upper level PTEs are present
        to_dtu_pte(get_pte_at(virt, 0))
    }
    else {
        // otherwise, walk through all levels
        get_pte_by_walk(virt, perm)
    }
}

pub struct AddrSpace {}

impl fmt::Debug for AddrSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fn print_as_rec(f: &mut fmt::Formatter<'_>, mut virt: u64, level: usize) {
            let mut ptes = get_pte_addr(virt, level as u32);
            for _ in 0..1 << cfg::LEVEL_BITS {
                let pte = unsafe { *(ptes as *const u64) };
                if pte != 0 {
                    let w = (cfg::LEVEL_CNT - level - 1) * 2;
                    writeln!(f, "{:w$}0x{:0>16x}: 0x{:0>16x}", "", virt, pte, w = w).ok();
                    if level > 0 {
                        print_as_rec(f, virt, level - 1);
                    }
                }

                virt += 1 << (level as usize * cfg::LEVEL_BITS + cfg::PAGE_BITS);
                ptes += 8;

                // don't enter the PTE area
                if virt >= 0x0800_0000_0000 {
                    break;
                }
            }
        }

        print_as_rec(f, 0, cfg::LEVEL_CNT - 1);
        Ok(())
    }
}
