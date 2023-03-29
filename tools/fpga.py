#!/usr/bin/env python3

import argparse
import traceback
from time import sleep, time
from datetime import datetime
import fcntl
import os
import sys
import select
import termios

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
DRAM_SIZE = 2 * 1024 * 1024 * 1024

KENV_ADDR = 0
KENV_SIZE = 4 * 1024
SERIAL_ADDR = KENV_ADDR + KENV_SIZE
SERIAL_SIZE = 4 * 1024
PMP_ADDR = SERIAL_ADDR + SERIAL_SIZE

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
    mod.mem.write_bytes(addr, bytes(buf), burst=False)  # TODO enable burst


def glob_addr(tile, offset):
    return (0x4000 + tile) << 49 | offset


def send_input(fpga_inst, chip, tile, ep, bytes):
    fpga_inst.nocif.send_bytes((chip, tile), ep, bytes)


def write_file(mod, file, offset):
    print("%s: loading %s with %u bytes to %#x" % (mod.name, file, os.path.getsize(file), offset))
    sys.stdout.flush()

    with open(file, "rb") as f:
        content = f.read()
    mod.mem.write_bytes_checked(offset, content, True)


def add_mod(dram, addr, mod, offset):
    (name, path) = mod.split('=')
    path = os.path.basename(path)
    size = os.path.getsize(path)
    write_u64(dram, offset + 0x0, glob_addr(MEM_TILE, addr))
    write_u64(dram, offset + 0x8, size)
    write_str(dram, name, offset + 16)
    write_file(dram, path, addr)
    return size


def tile_desc(tiles, i, vm):
    if i >= len(tiles):
        # mem size | TileAttr::IMEM | TileType::MEM
        return (DRAM_SIZE >> 12) << 28 | ((1 << 4) << 20) | 1

    tile_desc = 1 << 6  # RISCV
    if not vm:
        # mem size | TileAttr::IMEM
        tile_desc |= ((pmp_size >> 12) << 28) | ((1 << 4) << 20)
    if i < 5:
        tile_desc |= (1 << 1) << 20  # Rocket core
    else:
        tile_desc |= (1 << 0) << 20  # BOOM core
    if i == 6:
        tile_desc |= (1 << 2) << 20  # NIC
        tile_desc |= (1 << 3) << 20  # Serial
    return tile_desc


def load_boot_info(dram, mods, tiles, vm):
    # boot info
    kenv_off = KENV_ADDR
    write_u64(dram, kenv_off + 0 * 8, len(mods))    # mod_count
    write_u64(dram, kenv_off + 1 * 8, len(tiles) + 1)  # tile_count
    write_u64(dram, kenv_off + 2 * 8, 1)            # mem_count
    write_u64(dram, kenv_off + 3 * 8, 0)            # serv_count
    kenv_off += 8 * 4

    # mods
    mods_addr = PMP_ADDR + (len(tiles) * pmp_size)
    for m in mods:
        mod_size = add_mod(dram, mods_addr, m, kenv_off)
        mods_addr = (mods_addr + mod_size + 4096 - 1) & ~(4096 - 1)
        kenv_off += 80

    # tile descriptors
    for x in range(0, len(tiles)):
        write_u64(dram, kenv_off, tile_desc(tiles, x, vm))         # PM
        kenv_off += 8
    write_u64(dram, kenv_off, tile_desc(tiles, len(tiles), False))  # dram1
    kenv_off += 8

    # mems
    mem_start = mods_addr
    write_u64(dram, kenv_off + 0, glob_addr(MEM_TILE, mem_start))  # addr
    write_u64(dram, kenv_off + 8, DRAM_SIZE - mem_start)          # size


def load_prog(dram, tiles, i, args, vm):
    pm = tiles[i]
    print("%s: loading %s..." % (pm.name, args[0]))
    sys.stdout.flush()

    # start core
    pm.start()

    # reset TCU (clear command log and reset registers except FEATURES and EPs)
    pm.tcu_reset()

    # enable instruction trace for all tiles (doesn't cost anything)
    pm.rocket_enableTrace()

    # set features: privileged, vm, ctxsw
    pm.tcu_set_features(1, vm, 1)

    # invalidate all EPs
    for ep in range(0, 63):
        pm.tcu_set_ep(ep, EP.invalid())

    mem_begin = PMP_ADDR + i * pmp_size
    # install first PMP EP
    pmp_ep = MemEP()
    pmp_ep.set_chip(dram.mem.nocid[0])
    pmp_ep.set_tile(dram.mem.nocid[1])
    pmp_ep.set_act(0xFFFF)
    pmp_ep.set_flags(Flags.READ | Flags.WRITE)
    pmp_ep.set_addr(mem_begin)
    pmp_ep.set_size(pmp_size)
    pm.tcu_set_ep(0, pmp_ep)

    # verify entrypoint, because inject a jump instruction below that jumps to that address
    with open(args[0], 'rb') as f:
        elf = ELFFile(f)
        if elf.header['e_entry'] != 0x10001000:
            sys.exit("error: {} has entry {:#x}, not 0x10001000.".format(
                args[0], elf.header['e_entry']))

    # load ELF file
    dram.mem.write_elf(args[0], mem_begin - DRAM_OFF)
    sys.stdout.flush()

    argv = ENV + 0x400
    desc = tile_desc(tiles, i, vm)
    kenv = glob_addr(MEM_TILE, KENV_ADDR) if i == 0 else 0

    # init environment
    dram_env = ENV + mem_begin - DRAM_OFF
    write_u64(dram, dram_env - 8, 0x0000106f)  # j _start (+0x1000)
    write_u64(dram, dram_env + 0, 1)           # platform = HW
    write_u64(dram, dram_env + 8, i)           # chip, tile
    write_u64(dram, dram_env + 16, desc)       # tile_desc
    write_u64(dram, dram_env + 24, len(args))  # argc
    write_u64(dram, dram_env + 32, argv)       # argv
    write_u64(dram, dram_env + 40, 0)          # envp
    write_u64(dram, dram_env + 48, kenv)       # kenv
    write_u64(dram, dram_env + 56, len(tiles) + 1)  # raw tile count
    # tile ids
    env_off = 64
    for tile in tiles:
        write_u64(dram, dram_env + env_off, tile.nocid[0] << 8 | tile.nocid[1])
        env_off += 8
    write_u64(dram, dram_env + env_off, dram.mem.nocid[0] << 8 | dram.mem.nocid[1])

    # write arguments to memory
    args_addr = argv + len(args) * 8
    for (idx, a) in enumerate(args, 0):
        write_u64(dram, argv + (mem_begin - DRAM_OFF) + idx * 8, args_addr)
        write_str(dram, a, args_addr + mem_begin - DRAM_OFF)
        args_addr += (len(a) + 1 + 7) & ~7
        if args_addr > ENV + 0x800:
            sys.exit("Not enough space for arguments")

    sys.stdout.flush()


# inspired by MiniTerm (https://github.com/pyserial/pyserial/blob/master/serial/tools/miniterm.py)
class TCUTerm:
    def __init__(self, fpga_inst):
        self.fd = sys.stdin.fileno()
        # make stdin nonblocking
        fl = fcntl.fcntl(self.fd, fcntl.F_GETFL)
        fcntl.fcntl(self.fd, fcntl.F_SETFL, fl | os.O_NONBLOCK)
        # get original terminal attributes to restore them later
        self.old = termios.tcgetattr(self.fd)
        self.fpga_inst = fpga_inst
        # reset tile and EP in case they are set from a previous run
        write_u64(fpga_inst.dram1, SERIAL_ADDR + 0, 0)
        write_u64(fpga_inst.dram1, SERIAL_ADDR + 8, 0)

    def setup(self):
        new = termios.tcgetattr(self.fd)
        new[3] = new[3] & ~(termios.ICANON | termios.ISIG | termios.ECHO)
        new[6][termios.VMIN] = 1
        new[6][termios.VTIME] = 0
        termios.tcsetattr(self.fd, termios.TCSANOW, new)
        print("-- TCU Terminal ( Quit: Ctrl+] ) --")

    def getkey(self):
        try:
            # read multiple bytes to get sequences like ^[D
            bytes = sys.stdin.read(8)
        except KeyboardInterrupt:
            bytes = ['\x03']
        return bytes

    def write(self, c):
        bytes = c.encode('utf-8')
        # read desired destination
        tile = read_u64(self.fpga_inst.dram1, SERIAL_ADDR + 0)
        ep = read_u64(self.fpga_inst.dram1, SERIAL_ADDR + 8)
        # only send if it was initialized
        if ep != 0:
            send_input(self.fpga_inst, tile >> 8, tile & 0xFF, ep, bytes)

    def cleanup(self):
        termios.tcsetattr(self.fd, termios.TCSAFLUSH, self.old)


def main():
    mon = NoCmonitor()

    parser = argparse.ArgumentParser()
    parser.add_argument('--fpga', type=int)
    parser.add_argument('--version', type=int)
    parser.add_argument('--reset', action='store_true')
    parser.add_argument('--debug', type=int)
    parser.add_argument('--tile', action='append')
    parser.add_argument('--mod', action='append')
    parser.add_argument('--vm', action='store_true')
    parser.add_argument('--timeout', type=int)
    args = parser.parse_args()

    # connect to FPGA
    fpga_inst = fpga_top.FPGA_TOP(args.version, args.fpga, args.reset)

    # stop all tiles
    for tile in fpga_inst.pms:
        tile.stop()

    # check TCU versions
    for tile in fpga_inst.pms:
        tcu_version = tile.tcu_version()
        if tcu_version != args.version:
            print("Tile %s has TCU version %d, but expected %d" %
                  (tile.name, tcu_version, args.version))
            return

    # disable NoC ARQ for program upload
    for tile in fpga_inst.pms:
        tile.nocarq.set_arq_enable(0)
    fpga_inst.eth_rf.nocarq.set_arq_enable(0)
    fpga_inst.dram1.nocarq.set_arq_enable(0)
    fpga_inst.dram2.nocarq.set_arq_enable(0)

    global pmp_size
    pmp_size = 16 * 1024 * 1024 if args.vm else 64 * 1024 * 1024

    term = TCUTerm(fpga_inst)

    # load boot info into DRAM
    mods = [] if args.mod is None else args.mod
    load_boot_info(fpga_inst.dram1, mods, fpga_inst.pms, args.vm)

    # load programs onto tiles
    for i, pargs in enumerate(args.tile[0:len(fpga_inst.pms)], 0):
        load_prog(fpga_inst.dram1, fpga_inst.pms, i, pargs.split(' '), args.vm)

    # enable NoC ARQ when cores are running
    for tile in fpga_inst.pms:
        tile.nocarq.set_arq_enable(1)
        tile.nocarq.set_arq_timeout(200)  # reduce timeout
    fpga_inst.dram1.nocarq.set_arq_enable(1)
    fpga_inst.dram2.nocarq.set_arq_enable(1)

    # start tiles
    debug_tile = len(fpga_inst.pms) if args.debug is None else args.debug
    for i, tile in enumerate(fpga_inst.pms, 0):
        if i != debug_tile:
            # start core (via interrupt 0)
            fpga_inst.pms[i].rocket_start()

    # signal run.sh that everything has been loaded
    if args.debug is not None:
        ready = open('.ready', 'w')
        ready.write('1')
        ready.close()

    term.setup()

    # wait for prints
    start = int(time())
    timed_out = False
    try:
        while True:
            # check for timeout
            if args.timeout is not None and int(time()) - start >= args.timeout:
                print("Execution timed out after {} seconds".format(args.timeout))
                timed_out = True
                break

            # check if there is input to pass to the FPGA
            if sys.stdin in select.select([sys.stdin], [], [], 0)[0]:
                bytes = term.getkey()
                if len(bytes) == 1 and bytes[0] == chr(0x1d):
                    # force-extract logs on ctrl+]
                    timed_out = True
                    break
                term.write(bytes)

            # check for output
            try:
                bytes = fpga_inst.nocif.receive_bytes(timeout_ns=10_000_000)
            except:
                continue

            msg = ""
            try:
                msg = bytes.decode()
                sys.stdout.write(msg)
            except:
                print("Unable to decode: {}".format(bytes))
            sys.stdout.write('\033[0m')
            sys.stdout.flush()
            if "Shutting down" in msg:
                break
    except KeyboardInterrupt:
        timed_out = True

    term.cleanup()

    # disable NoC ARQ again for post-processing
    for tile in fpga_inst.pms:
        tile.nocarq.set_arq_enable(0)
    fpga_inst.dram1.nocarq.set_arq_enable(0)
    fpga_inst.dram2.nocarq.set_arq_enable(0)

    # stop all tiles
    print("Stopping all tiles...")
    for i, tile in enumerate(fpga_inst.pms, 0):
        try:
            dropped_packets = tile.nocarq.get_arq_drop_packet_count()
            total_packets = tile.nocarq.get_arq_packet_count()
            print("PM{}: NoC dropped/total packets: {}/{} ({:.0f}%)".format(i,
                  dropped_packets, total_packets, dropped_packets/total_packets*100))
        except Exception as e:
            print("PM{}: unable to read number of dropped NoC packets: {}".format(i, e))

        try:
            print("PM{}: TCU dropped/error flits: {}/{}".format(i,
                  tile.tcu_drop_flit_count(), tile.tcu_error_flit_count()))
        except Exception as e:
            print("PM{}: unable to read number of TCU dropped flits: {}".format(i, e))

        # extract TCU log on timeouts
        if timed_out:
            print("PM{}: reading TCU log...".format(i))
            sys.stdout.flush()
            try:
                tile.tcu_print_log('log/pm' + str(i) + '-tcu-cmds.log')
            except Exception as e:
                print("PM{}: unable to read TCU log: {}".format(i, e))
                print("PM{}: resetting TCU and reading all logs...".format(i))
                sys.stdout.flush()
                tile.tcu_reset()
                try:
                    tile.tcu_print_log('log/pm' + str(i) + '-tcu-cmds.log', all=True)
                except:
                    pass

        # extract instruction trace
        try:
            tile.rocket_printTrace('log/pm' + str(i) + '-instrs.log')
        except Exception as e:
            print("PM{}: unable to read instruction trace: {}".format(i, e))
            print("PM{}: resetting TCU and reading all logs...".format(i))
            sys.stdout.flush()
            tile.tcu_reset()
            try:
                tile.rocket_printTrace('log/pm' + str(i) + '-instrs.log', all=True)
            except:
                pass

        tile.stop()


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
