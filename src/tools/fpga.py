#!/usr/bin/env python3

import argparse
import traceback
from time import sleep, time
from datetime import datetime
import os, sys, select

import modids
import fpga_top
from elftools.elf.elffile import ELFFile
from noc import NoCmonitor
from tcu import EP, MemEP, Flags
from fpga_utils import FPGA_Error
import memory

DRAM_OFF = 0x10000000
ENV = 0x10000008
MEM_TILE = 8
MEM_SIZE = 2 * 1024 * 1024
DRAM_SIZE = 2 * 1024 * 1024 * 1024
MAX_FS_SIZE = 256 * 1024 * 1024
KENV_SIZE = 16 * 1024 * 1024
SERIAL_SIZE = 4 * 1024

serial_begin = 0
pmp_size = 0

def read_u64(mod, addr):
    return mod.mem[addr]

def write_u64(mod, addr, value):
    mod.mem[addr] = value

def read_str(mod, addr, length):
    b = mod.mem.read_bytes(addr, length)
    return b.decode()

def write_str(mod, str, addr):
    buf = bytearray(str.encode())
    buf += b'\x00'
    mod.mem.write_bytes(addr, bytes(buf), burst=False) # TODO enable burst

def glob_addr(pe, offset):
    return (0x80 + pe) << 56 | offset

def send_input(dram, line):
    global serial_begin
    while read_u64(dram, serial_begin) != 0:
        pass
    write_str(dram, line, serial_begin + 8)
    write_u64(dram, serial_begin, len(line))

def write_file(mod, file, offset):
    print("%s: loading %u bytes to %#x" % (mod.name, os.path.getsize(file), offset))
    sys.stdout.flush()

    with open(file, "rb") as f:
        content = f.read()
    mod.mem.write_bytes_checked(offset, content, True)

def add_mod(dram, addr, name, offset):
    size = os.path.getsize(name)
    write_u64(dram, offset + 0x0, glob_addr(MEM_TILE, addr))
    write_u64(dram, offset + 0x8, size)
    write_str(dram, name, offset + 16)
    write_file(dram, name, addr)
    return size

def pe_desc(i, vm):
    pe_desc = (3 << 3) | 1 if vm else pmp_size | (3 << 3) | 0
    if i < 5:
        pe_desc |= 1 << 8 # Rocket core
    else:
        pe_desc |= 1 << 7 # BOOM core
    if i == 6:
        pe_desc |= 1 << 9 # NIC
    return pe_desc

def load_boot_info(dram, mods, pes, vm):
    info_start = MAX_FS_SIZE + len(pes) * pmp_size

    # boot info
    kenv_off = info_start
    write_u64(dram, kenv_off + 0 * 8, len(mods))    # mod_count
    write_u64(dram, kenv_off + 1 * 8, len(pes) + 1) # pe_count
    write_u64(dram, kenv_off + 2 * 8, 1)            # mem_count
    write_u64(dram, kenv_off + 3 * 8, 0)            # serv_count
    kenv_off += 8 * 4

    # mods
    mods_addr = info_start + 0x1000
    for m in mods:
        mod_size = add_mod(dram, mods_addr, m, kenv_off)
        mods_addr = (mods_addr + mod_size + 4096 - 1) & ~(4096 - 1)
        kenv_off += 80

    # PEs
    for x in range(0, len(pes)):
        write_u64(dram, kenv_off, pe_desc(x, vm))       # PM
        kenv_off += 8
    write_u64(dram, kenv_off, DRAM_SIZE | (0 << 3) | 2) # dram
    kenv_off += 8

    # mems
    mem_start = info_start + KENV_SIZE + SERIAL_SIZE
    write_u64(dram, kenv_off + 0, mem_start)             # addr (ignored)
    write_u64(dram, kenv_off + 8, DRAM_SIZE - mem_start) # size

def load_prog(dram, pms, i, args, vm):
    pm = pms[i]
    print("%s: loading %s..." % (pm.name, args[0]))
    sys.stdout.flush()

    # start core
    pm.start()

    # reset TCU (clear command log and reset registers except FEATURES and EPs)
    pm.tcu_reset()

    # enable instruction trace for all PEs (doesn't cost anything)
    pm.rocket_enableTrace()

    # set features: privileged, vm, ctxsw
    pm.tcu_set_features(1, vm, 1)

    # invalidate all EPs
    for ep in range(0, 63):
        pm.tcu_set_ep(ep, EP.invalid())

    mem_begin = MAX_FS_SIZE + i * pmp_size
    # install first PMP EP
    pmp_ep = MemEP()
    pmp_ep.set_pe(dram.mem.nocid[1])
    pmp_ep.set_vpe(0xFFFF)
    pmp_ep.set_flags(Flags.READ | Flags.WRITE)
    pmp_ep.set_addr(mem_begin)
    pmp_ep.set_size(pmp_size)
    pm.tcu_set_ep(0, pmp_ep)
    # install EP for serial input
    global serial_begin
    ser_ep = MemEP()
    ser_ep.set_pe(dram.mem.nocid[1])
    ser_ep.set_vpe(0xFFFF)
    ser_ep.set_flags(Flags.READ | Flags.WRITE)
    ser_ep.set_addr(serial_begin)
    ser_ep.set_size(SERIAL_SIZE)
    pm.tcu_set_ep(127, ser_ep)

    # verify entrypoint, because inject a jump instruction below that jumps to that address
    with open(args[0], 'rb') as f:
        elf = ELFFile(f)
        if elf.header['e_entry'] != 0x10001000:
            sys.exit("error: {} has entry {:#x}, not 0x10001000.".format(args[0], elf.header['e_entry']))

    # load ELF file
    dram.mem.write_elf(args[0], mem_begin - DRAM_OFF)
    sys.stdout.flush()

    argv = ENV + 0x400
    if vm:
        heap_size = 0x10000
    else:
        heap_size = 0
    desc = pe_desc(i, vm)
    kenv = glob_addr(MEM_TILE, MAX_FS_SIZE + len(pms) * pmp_size) if i == 0 else 0

    # init environment
    dram_env = ENV + mem_begin - DRAM_OFF
    write_u64(dram, dram_env - 8, 0x0000106f)  # j _start (+0x1000)
    write_u64(dram, dram_env + 0, 1)           # platform = HW
    write_u64(dram, dram_env + 8, i)           # pe_id
    write_u64(dram, dram_env + 16, desc)       # pe_desc
    write_u64(dram, dram_env + 24, len(args))  # argc
    write_u64(dram, dram_env + 32, argv)       # argv
    write_u64(dram, dram_env + 40, heap_size)  # heap size
    write_u64(dram, dram_env + 48, kenv)       # kenv
    write_u64(dram, dram_env + 56, 0)          # lambda

    # write arguments to memory
    args_addr = argv + len(args) * 8
    for (idx, a) in enumerate(args, 0):
        write_u64(dram, argv + (mem_begin - DRAM_OFF) + idx * 8, args_addr)
        write_str(dram, a, args_addr + mem_begin - DRAM_OFF)
        args_addr += (len(a) + 1 + 7) & ~7
        if args_addr > ENV + 0x800:
            sys.exit("Not enough space for arguments")

    sys.stdout.flush()

def main():
    mon = NoCmonitor()

    parser = argparse.ArgumentParser()
    parser.add_argument('--fpga', type=int)
    parser.add_argument('--reset', action='store_true')
    parser.add_argument('--debug', type=int)
    parser.add_argument('--pe', action='append')
    parser.add_argument('--mod', action='append')
    parser.add_argument('--vm', action='store_true')
    parser.add_argument('--timeout', type=int)
    parser.add_argument('--fs')
    args = parser.parse_args()

    # connect to FPGA
    fpga_inst = fpga_top.FPGA_TOP(args.fpga)
    if args.reset:
        fpga_inst.eth_rf.system_reset()

    # stop all PEs
    for pe in fpga_inst.pms:
        pe.stop()

    # disable NoC ARQ for program upload
    for pe in fpga_inst.pms:
        pe.nocarq.set_arq_enable(0)
    fpga_inst.eth_rf.nocarq.set_arq_enable(0)
    fpga_inst.dram1.nocarq.set_arq_enable(0)
    fpga_inst.dram2.nocarq.set_arq_enable(0)

    global serial_begin, pmp_size
    pmp_size = 8 * 1024 * 1024 if args.vm else 32 * 1024 * 1024
    serial_begin = MAX_FS_SIZE + len(fpga_inst.pms) * pmp_size + KENV_SIZE

    # load boot info into DRAM
    mods = [] if args.mod is None else args.mod
    load_boot_info(fpga_inst.dram1, mods, fpga_inst.pms, args.vm)

    # load file system into DRAM, if there is any
    if not args.fs is None:
        write_file(fpga_inst.dram1, args.fs, 0)

    # load programs onto PEs
    for i, peargs in enumerate(args.pe[0:len(fpga_inst.pms)], 0):
        load_prog(fpga_inst.dram1, fpga_inst.pms, i, peargs.split(' '), args.vm)

    # enable NoC ARQ when cores are running
    for pe in fpga_inst.pms:
        pe.nocarq.set_arq_enable(1)
        pe.nocarq.set_arq_timeout(200)    #reduce timeout
    fpga_inst.dram1.nocarq.set_arq_enable(1)
    fpga_inst.dram2.nocarq.set_arq_enable(1)

    # start PEs
    debug_pe = len(fpga_inst.pms) if args.debug is None else args.debug
    for i, pe in enumerate(fpga_inst.pms, 0):
        if i != debug_pe:
            # start core (via interrupt 0)
            fpga_inst.pms[i].rocket_start()

    # signal run.sh that everything has been loaded
    if not args.debug is None:
        ready = open('.ready', 'w')
        ready.write('1')
        ready.close()

    # wait for prints
    start = int(time())
    timed_out = False
    try:
        while True:
            # check for timeout
            if not args.timeout is None and int(time()) - start >= args.timeout:
                print("Execution timed out after {} seconds".format(args.timeout))
                timed_out = True
                break

            # check if there is input to pass to the FPGA
            while sys.stdin in select.select([sys.stdin], [], [], 0)[0]:
                line = sys.stdin.readline()
                if line:
                    send_input(fpga_inst.dram1, line)

            # check for output
            try:
                bytes = fpga_inst.nocif.receive_bytes(timeout_ns = 10_000_000)
            except KeyboardInterrupt:
                # force-extract logs on ^C
                timed_out = True
                break
            except:
                continue

            msg = ""
            try:
                msg = bytes.decode()
                sys.stdout.write(msg)
            except KeyboardInterrupt:
                timed_out = True
                break
            except:
                print("Unable to decode: {}".format(bytes))
            sys.stdout.write('\033[0m')
            sys.stdout.flush()
            if "Shutting down" in msg:
                break
    except KeyboardInterrupt:
        timed_out = True

    # disable NoC ARQ again for post-processing
    for pe in fpga_inst.pms:
        pe.nocarq.set_arq_enable(0)
    fpga_inst.dram1.nocarq.set_arq_enable(0)
    fpga_inst.dram2.nocarq.set_arq_enable(0)

    # stop all PEs
    print("Stopping all PEs...")
    for i, pe in enumerate(fpga_inst.pms, 0):
        try:
            dropped_packets = pe.nocarq.get_arq_drop_packet_count()
            total_packets = pe.nocarq.get_arq_packet_count()
            print("PM{}: NoC dropped/total packets: {}/{} ({:.0f}%)".format(i, dropped_packets, total_packets, dropped_packets/total_packets*100))
        except Exception as e:
            print("PM{}: unable to read number of dropped NoC packets: {}".format(i, e))

        try:
            print("PM{}: TCU dropped/error flits: {}/{}".format(i, pe.tcu_drop_flit_count(), pe.tcu_error_flit_count()))
        except Exception as e:
            print("PM{}: unable to read number of TCU dropped flits: {}".format(i, e))

        # extract TCU log on timeouts
        if timed_out:
            print("PM{}: reading TCU log...".format(i))
            sys.stdout.flush()
            try:
                pe.tcu_print_log('log/pm' + str(i) + '-tcu-cmds.log')
            except Exception as e:
                print("PM{}: unable to read TCU log: {}".format(i, e))
                print("PM{}: resetting TCU and reading all logs...".format(i))
                sys.stdout.flush()
                pe.tcu_reset()
                try:
                    pe.tcu_print_log('log/pm' + str(i) + '-tcu-cmds.log', all=True)
                except:
                    pass

        # extract instruction trace
        try:
            pe.rocket_printTrace('log/pm' + str(i) + '-instrs.log')
        except Exception as e:
            print("PM{}: unable to read instruction trace: {}".format(i, e))
            print("PM{}: resetting TCU and reading all logs...".format(i))
            sys.stdout.flush()
            pe.tcu_reset()
            try:
                pe.rocket_printTrace('log/pm' + str(i) + '-instrs.log', all=True)
            except:
                pass

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
    pass
