#!/usr/bin/env python3

import sys
import subprocess
import re
from os.path import basename

if len(sys.argv) < 2:
    print("Usage: %s <act_id>" % sys.argv[0])
    sys.exit(1)

id = int(sys.argv[1])
cur_act = -1
cur_tile = ""

while True:
    line = sys.stdin.readline()
    if not line:
        break

    old_act = cur_act

    if "ACT_ID" in line:
        m = re.match(r'.*(tile[0-9]+\.).*TCU\[ACT_ID\s*\]: 0x([0-9a-f]+).*', line)
        if m:
            next_act = int(m[2], 16)
            next_tile = m[1]
            if cur_act != id or (cur_act == id and next_tile == cur_tile):
                cur_act = next_act
                cur_tile = next_tile

    if old_act != cur_act:
        print("------ Context Switch from %d to %d on %s ------" % (old_act, cur_act, cur_tile))
    if "PRINT: " in line or (cur_act == id and cur_tile in line):
        print(line.rstrip())
