#!/usr/bin/env python3

import argparse
import traceback
from time import sleep
from datetime import datetime
import os, sys

import modids
import fpga_top
from noc import NoCmonitor
from fpga_utils import FPGA_Error
import memory

ENV = 0x10100000
SERIAL_BUF = 0x101F0008
SERIAL_BUFSIZE = 0x1000 - 8
SERIAL_ACK = 0x101F0000
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

def fetch_print(pm):
    length = read_u64(pm, SERIAL_ACK) & 0xFFFFFFFF
    if length != 0 and length <= SERIAL_BUFSIZE:
        line = read_str(pm, SERIAL_BUF, length)
        sys.stdout.write(line)
        sys.stdout.write('\033[0m')
        sys.stdout.flush()

        write_u64(pm, SERIAL_ACK, 0)
        if "Shutting down" in line:
            return 2
        return 1
    elif length != 0:
        print("Got invalid length from %s: %u" % (pm.name, length))
    return 0

def glob_addr(pe, offset):
    return (0x80 + pe) << 56 | offset

def write_file(dram, file, offset):
    print("%s: loading %u bytes to %#x" % (dram.name, os.path.getsize(file), offset))
    sys.stdout.flush()

    with open(file, "rb") as f:
        content = f.read()
    dram.mem.write_bytes_checked(offset, content, True)

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
    for (i, a) in enumerate(args, 0):
        write_u64(pm, argv + i * 8, args_addr)
        write_str(pm, a, args_addr)
        args_addr += (len(a) + 1 + 7) & ~7
        if args_addr > ENV + 0x800:
            sys.exit("Not enough space for arguments")

    # start core (via interrupt 0)
    pm.rocket_start()

def main():
    # get connection to FPGA, SW12=0000b -> chipid=0
    fpga_inst = fpga_top.FPGA_TOP(0)
    # fpga_inst.eth_rf.system_reset()

    mon = NoCmonitor()

    parser = argparse.ArgumentParser()
    parser.add_argument('--pe', action='append')
    parser.add_argument('--mod', action='append')
    args = parser.parse_args()

    mods = [] if args.mod is None else args.mod

    # load boot info into DRAM
    load_boot_info(fpga_inst.dram1, mods)

    # load programs onto PEs
    pms = [fpga_inst.pm6, fpga_inst.pm7, fpga_inst.pm3, fpga_inst.pm5]
    for i, peargs in enumerate(args.pe, 0):
        load_prog(pms[i], i, peargs.split(' '))

    # wait for prints
    run = True
    while run:
        counter = 0
        for pm in pms:
            res = fetch_print(pm)
            if res == 2:
                run = False
            else:
                counter += res

    # stop all PEs
    print("Stopping all PEs...")
    for pm in pms:
        pm.stop()

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
