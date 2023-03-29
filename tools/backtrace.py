#!/usr/bin/env python3

import os
import re
import subprocess
import sys
from collections import OrderedDict
from shlex import quote

if len(sys.argv) != 3:
    sys.exit("Usage: {} <crossprefix> <binary>".format(sys.argv[0]))

crossprefix = sys.argv[1]
binary = sys.argv[2]

regex_symbol = re.compile(r'^([0-9a-fA-F]*)\s+([BdDdTtVvWwuU])\s+(.*)$')
regex_btline = re.compile(r'^(?:.*?\[[^\]]+\])?\s*(?:0x)?([0-9a-f]+)\s*$')
regex_sanbtline = re.compile(r'^\s*#\d+\s+0x([0-9a-f]+).*')


def get_location(addr):
    cmd = ["addr2line", "-e", binary, "{:#x}".format(addr)]
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE)
    line = proc.stdout.readline()
    return line.decode(errors='ignore').replace(os.environ.get('PWD'), '.')


def find_sym(addr):
    last_addr = 0
    for s in syms:
        if s >= addr:
            return syms[last_addr] if last_addr != 0 else {}
        last_addr = s
    return {}


def print_func(addr):
    # hack for Linux: currently, we generate PIE binaries and thus, Linux puts code and data at
    # weird addresses. with setarch -R, Linux uses the fixed offset 0x555555554000.
    if "/host-" in binary:
        addr -= 0x555555554000

    sym = find_sym(addr)
    if len(sym) == 0:
        return

    loc = get_location(addr)
    print(" {:#x} {}({}) + {:#x} = {:#x} in {}"
          .format(addr, sym['name'], sym['sec'], addr - sym['addr'], sym['addr'], loc))


# scan binary
syms = {}
cmd = "{}nm {} | c++filt".format(quote(crossprefix), quote(binary))
proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, shell=True)
for line in proc.stdout.readlines():
    line = line.strip().decode(errors='ignore')
    match = regex_symbol.match(line)
    if match:
        addr = int(match.group(1), 16)
        sec = match.group(2)
        sym = match.group(3)
        syms[addr] = {'addr': addr, 'sec': sec, 'name': sym}

# sort symbols by address
syms = OrderedDict(sorted(syms.items(), key=lambda t: t[0]))

print("Scanning binary done, reading backtrace from stdin...")

# decode backtrace
for line in sys.stdin:
    match = regex_btline.match(line.strip())
    if not match:
        match = regex_sanbtline.match(line.strip())
    if match:
        print_func(int(match.group(1), 16))
