#!/usr/bin/env python3

import os
import re
import sys

if len(sys.argv) < 2:
    print("Usage: {} <log-file>...".format(sys.argv[0]))
    print("  The tool assumes that the file names match the pattern ^pm\\d+-.*$")
    sys.exit(1)

events = []
for file in sys.argv[1:]:
    m = re.search(r"^pm(\d+)-.*$", os.path.basename(file))
    pm = int(m[1])

    f = open(file, 'r')
    for line in f.readlines():
        m = re.search(r"^\s*(\d+): Time:\s*(\d+), (.*)$", line)
        if m:
            events.append((pm, int(m[2]), m[3]))


def sort_by_ts(event):
    return event[1]


colors = ["31", "32", "33", "34", "35", "36"]
events.sort(key=sort_by_ts)
for ev in events:
    color = colors[ev[0] % len(colors)]
    print("\x1B[0;{}m[PM{} @ {:>12}] {}\x1B[0m".format(color, ev[0], ev[1], ev[2]))
