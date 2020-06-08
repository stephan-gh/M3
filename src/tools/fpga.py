#!/usr/bin/env python3

import traceback
from time import sleep
from datetime import datetime
import sys

import modids
from noc import noc_packet
import fpga_top
from fpga_utils import FPGA_Error

ENV = 0x10100000
SERIAL_BUF = 0x101F0008
SERIAL_BUFSIZE = 0x1000 - 8
SERIAL_ACK = 0x101F0000
MEM_SIZE = 2 * 1024 * 1024

def read64bit(pm, addr):
    return pm.mem[addr]

def write64bit(pm, addr, value):
    pm.mem[addr] = value

def readStr(mod, addr, length):
    line = ""
    length = (length + 7) & ~7
    for off in range(0, length, 8):
        val = read64bit(mod, addr + off)
        for x in range(0, 64, 8):
            hexdig = (val >> x) & 0xFF
            if hexdig == 0:
                break
            if (hexdig < 0x20 or hexdig > 0x80) and hexdig != ord('\t') and hexdig != ord('\n') and hexdig != 0x1b:
                hexdig = ord('?')
            line += chr(hexdig)
    return line

def fetchPrint(pm):
    length = read64bit(pm, SERIAL_ACK) & 0xFFFFFFFF
    if length != 0 and length <= SERIAL_BUFSIZE:
        line = readStr(pm, SERIAL_BUF, length)
        sys.stdout.write(line)
        sys.stdout.write('\033[0m')
        sys.stdout.flush()

        write64bit(pm, SERIAL_ACK, 0)
        if "kernel" in line and "Shutting down" in line:
            return 2
        return 1
    elif length != 0:
        print("Got invalid length from %s: %u" % (pm.name, length))
    return 0

def load_prog(pm, i, memfile):
    print("%s: loading %s..." % (pm.name, memfile))

    # first disable core to start from initial state
    pm.stop()

    # start core
    pm.start()

    # init mem
    pm.initMem(memfile)

    pm.mem[ENV + 0] = 1 # platform = HW
    pm.mem[ENV + 8] = i # pe_id
    pe_desc = MEM_SIZE | (3 << 3) | 0
    pm.mem[ENV + 16] = (0 << 32) | pe_desc # argc | pe_desc
    pm.mem[ENV + 24] = 0 # argv
    pm.mem[ENV + 32] = 0 # heap size
    pm.mem[ENV + 40] = 0 # pe_mem_base
    pm.mem[ENV + 48] = 0 # pe_mem_size
    pm.mem[ENV + 56] = 0 # kemv
    pm.mem[ENV + 64] = 0 # lambda

    # start core (via interrupt 0)
    pm.rocket_setInt(0, 1)
    pm.rocket_setInt(0, 0)

def main():
    # get connection to FPGA, SW12=0000b -> chipid=0
    fpga_inst = fpga_top.FPGA_TOP(0)

    # load programs onto PEs
    all_pms = [fpga_inst.pm6, fpga_inst.pm7, fpga_inst.pm3, fpga_inst.pm5]
    pms = all_pms[0:len(sys.argv) - 1]
    for i, prog in enumerate(sys.argv[1:], 0):
        load_prog(pms[i], i, prog)

    # wait for prints
    run = True
    start = datetime.now()
    while run and (datetime.now() - start).seconds < 1:
        counter = 0
        for pm in pms:
            res = fetchPrint(pm)
            if res == 2:
                run = False
            else:
                counter += res

        # if nobody wanted to print something, take a break
        if counter == 0:
            sleep(0.01)

    # stop all PEs
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
