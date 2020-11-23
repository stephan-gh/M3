#!/usr/bin/env python3

import argparse
import traceback
from time import sleep
from datetime import datetime
import os, sys

import modids
import fpga_top
from noc import NoCmonitor
from tcu import MemEP, Flags
from fpga_utils import FPGA_Error
import memory

ENV = 0x10100000
MEM_SIZE = 2 * 1024 * 1024
DRAM_SIZE = 2 * 1024 * 1024 * 1024
MAX_FS_SIZE = 256 * 1024 * 1024
KENV_SIZE = 16 * 1024 * 1024

def read_u64(pm, addr):
    return pm.mem[addr]

def write_u64(pm, addr, value):
    pm.mem[addr] = value

def read_str(mod, addr, length):
    b = mod.mem.read_bytes(addr, length)
    return b.decode()

def write_str(pm, str, addr):
    buf = bytearray(str.encode())
    buf += b'\x00'
    pm.mem.write_bytes(addr, bytes(buf), burst=False) # TODO enable burst

def glob_addr(pe, offset):
    return (0x80 + pe) << 56 | offset

def write_file(mod, file, offset):
    print("%s: loading %u bytes to %#x" % (mod.name, os.path.getsize(file), offset))
    sys.stdout.flush()

    with open(file, "rb") as f:
        content = f.read()
    mod.mem.write_bytes_checked(offset, content, True)

def add_mod(dram, addr, name, offset):
    size = os.path.getsize(name)
    write_u64(dram, offset + 0x0, glob_addr(4, addr))
    write_u64(dram, offset + 0x8, size)
    write_str(dram, name, offset + 16)
    write_file(dram, name, addr)
    return size

def load_boot_info(dram, mods):
    # boot info
    kenv_off = MAX_FS_SIZE
    write_u64(dram, kenv_off + 0 * 8, len(mods)) # mod_count
    write_u64(dram, kenv_off + 1 * 8, 5)         # pe_count
    write_u64(dram, kenv_off + 2 * 8, 1)         # mem_count
    write_u64(dram, kenv_off + 3 * 8, 0)         # serv_count
    kenv_off += 8 * 4

    # mods
    mods_addr = MAX_FS_SIZE + 0x1000
    for m in mods:
        mod_size = add_mod(dram, mods_addr, m, kenv_off)
        mods_addr = (mods_addr + mod_size + 4096 - 1) & ~(4096 - 1)
        kenv_off += 80

    # PEs
    for x in range(0, 4):
        write_u64(dram, kenv_off, MEM_SIZE | (3 << 3) | 0) # PM
        kenv_off += 8
    write_u64(dram, kenv_off, DRAM_SIZE | (0 << 3) | 2) # dram
    kenv_off += 8

    # mems
    write_u64(dram, kenv_off, MAX_FS_SIZE + KENV_SIZE) # addr (ignored)
    kenv_off += 8
    write_u64(dram, kenv_off, DRAM_SIZE - MAX_FS_SIZE - KENV_SIZE) # size
    kenv_off += 8

def load_prog(pm, i, args):
    print("%s: loading %s..." % (pm.name, args[0]))
    sys.stdout.flush()

    # first disable core to start from initial state
    pm.stop()

    # start core
    pm.start()

    # make privileged
    pm.tcu_set_privileged(1)

    # install EP for prints
    print_ep = MemEP()
    print_ep.set_pe(modids.MODID_ETH)
    print_ep.set_flags(Flags.WRITE)
    print_ep.set_size(256)
    pm.tcu_set_ep(63, print_ep)

    # load ELF file
    pm.mem.write_elf(args[0])
    sys.stdout.flush()

    argv = ENV + 0x400
    pe_desc = MEM_SIZE | (3 << 3) | 0
    kenv = glob_addr(4, MAX_FS_SIZE) if i == 0 else 0

    # init environment
    write_u64(pm, ENV + 0, 1)           # platform = HW
    write_u64(pm, ENV + 8, i)           # pe_id
    write_u64(pm, ENV + 16, pe_desc)    # pe_desc
    write_u64(pm, ENV + 24, len(args))  # argc
    write_u64(pm, ENV + 32, argv)       # argv
    write_u64(pm, ENV + 40, 0)          # heap size
    write_u64(pm, ENV + 48, 0)          # pe_mem_base
    write_u64(pm, ENV + 56, 0)          # pe_mem_size
    write_u64(pm, ENV + 64, kenv)       # kenv
    write_u64(pm, ENV + 72, 0)          # lambda

    # write arguments to memory
    args_addr = argv + len(args) * 8
    for (idx, a) in enumerate(args, 0):
        write_u64(pm, argv + idx * 8, args_addr)
        write_str(pm, a, args_addr)
        args_addr += (len(a) + 1 + 7) & ~7
        if args_addr > ENV + 0x800:
            sys.exit("Not enough space for arguments")

    # start core (via interrupt 0)
    pm.rocket_start()

def main():
    mon = NoCmonitor()

    parser = argparse.ArgumentParser()
    parser.add_argument('--fpga', type=int)
    parser.add_argument('--reset', action='store_true')
    parser.add_argument('--pe', action='append')
    parser.add_argument('--mod', action='append')
    parser.add_argument('--fs')
    args = parser.parse_args()

    # connect to FPGA
    fpga_inst = fpga_top.FPGA_TOP(args.fpga)
    if args.reset:
        fpga_inst.eth_rf.system_reset()

    # load boot info into DRAM
    mods = [] if args.mod is None else args.mod
    load_boot_info(fpga_inst.dram1, mods)

    # load file system into DRAM, if there is any
    if not args.fs is None:
        write_file(fpga_inst.dram1, args.fs, 0)

    # load programs onto PEs
    pes = [fpga_inst.pm6, fpga_inst.pm7, fpga_inst.pm3, fpga_inst.pm5]
    for i, peargs in enumerate(args.pe, 0):
        load_prog(pes[i], i, peargs.split(' '))

    # wait for prints
    while True:
        msg = fpga_inst.nocif.receive_bytes().decode()
        sys.stdout.write(msg)
        sys.stdout.write('\033[0m')
        sys.stdout.flush()
        if "Shutting down" in msg:
            break

    # stop all PEs
    print("Stopping all PEs...")
    for pe in pes:
        pe.stop()

try:
    main()
except FPGA_Error as e:
    sys.stdout.flush()
    traceback.print_exc()
except Exception:
    sys.stdout.flush()
    traceback.print_exc()
except KeyboardInterrupt:
    print("interrupt")
