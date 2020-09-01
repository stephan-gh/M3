#!/usr/bin/env python3

import argparse
import traceback
from time import sleep
from datetime import datetime
import os, sys

import modids
from noc import noc_packet
import fpga_top
from fpga_utils import FPGA_Error

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

def begin_to_u64(str):
    res = 0
    i = 0
    for x in range(0, 16, 2):
        if i >= len(str):
            break
        res |= ord(str[i]) << x * 4
        i += 1
    return res

def string_to_u64s(str):
    vals = []
    for i in range(0, len(str), 8):
        int64 = begin_to_u64(str[i:i + 8])
        vals += [int64]
    return vals

def read_str(mod, addr, length):
    line = ""
    length = (length + 7) & ~7
    for off in range(0, length, 8):
        val = read_u64(mod, addr + off)
        for x in range(0, 64, 8):
            hexdig = (val >> x) & 0xFF
            if hexdig == 0:
                break
            if (hexdig < 0x20 or hexdig > 0x80) and hexdig != ord('\t') and hexdig != ord('\n') and hexdig != 0x1b:
                hexdig = ord('?')
            line += chr(hexdig)
    return line

def write_str(pm, str, addr):
    for v in string_to_u64s(str):
        write_u64(pm, addr, v)
        addr += 8
    write_u64(pm, addr, 0)

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
    with open(file, "rb") as f:
        while True:
            data = f.read(8)
            if data == b"":
                break
            u64 = int.from_bytes(data, byteorder='little')
            write_u64(dram, offset, u64)
            offset += 8

def add_mod(dram, addr, name, offset):
    size = os.path.getsize(name)
    write_u64(dram, offset + 0x0, glob_addr(4, addr))
    write_u64(dram, offset + 0x8, size)
    write_str(dram, name, offset + 16)
    write_file(dram, name, addr)
    return size

def load_prog(pm, i, dram, args, mods):
    memfile = args[0] + ".hex"
    print("%s: loading %s..." % (pm.name, memfile))
    sys.stdout.flush()

    # first disable core to start from initial state
    pm.stop()

    # start core
    pm.start()

    # init mem
    pm.initMem(memfile)

    # init kernel environment
    if i == 0:
        kenv = MAX_FS_SIZE

        # boot info
        kenv_off = kenv
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
        kenv = glob_addr(4, kenv)
    else:
        kenv = 0

    # init environment
    argv = ENV + 0x400
    pe_desc = MEM_SIZE | (3 << 3) | 0
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

    parser = argparse.ArgumentParser()
    parser.add_argument('--pe', action='append')
    parser.add_argument('--mod', action='append')
    args = parser.parse_args()

    mods = [] if args.mod is None else args.mod

    # load programs onto PEs
    pms = [fpga_inst.pm6, fpga_inst.pm7, fpga_inst.pm3, fpga_inst.pm5]
    for i, peargs in enumerate(args.pe, 0):
        load_prog(pms[i], i, fpga_inst.dram1, peargs.split(' '), mods)

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
    traceback.print_exc()
except Exception:
    traceback.print_exc()
except KeyboardInterrupt:
    print("interrupt")
