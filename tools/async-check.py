#!/usr/bin/env python3

from fnmatch import fnmatch
import os
import re
import sys


def find_files(dir, pat):
    res = []
    for root, dirs, files in os.walk(dir):
        for f in files:
            if fnmatch(f, pat):
                res.append(os.path.join(root, f))
    return res


def file_content(file):
    with open(file, 'r') as f:
        return f.read()


class Location:
    def __init__(self, file, line):
        self.file = file
        self.line = line

    def __str__(self):
        return self.file + ":" + str(self.line)


class FuncCall:
    def __init__(self, name, loc):
        self.name = name
        self.loc = loc

    def __str__(self):
        return self.name + " at " + str(self.loc)


class FuncDef:
    def __init__(self, name, loc):
        self.name = name
        self.loc = loc
        self.calls = []

    def __str__(self):
        return self.name + " at " + str(self.loc)


def parse_file(file):
    bytes = file_content(file)

    func = None
    funcs = []

    line = 1
    pos = 0
    braces = 0
    in_func = False
    while pos < len(bytes):
        if bytes[pos] == '\n':
            line += 1
            pos += 1
        elif not in_func:
            m = re.search(r'\bfn\s+([a-zA-Z0-9_]+)\s*(?:<.*?>\s*)?\(', bytes[pos:])
            if m:
                line += bytes[pos:pos + m.span()[1]].count('\n')
                pos += m.span()[1]
                func = FuncDef(m.group(1), Location(file, line))
                braces = 0
                in_func = True
            else:
                break
        else:
            if bytes[pos] == '{':
                braces += 1
            elif (braces == 0 and bytes[pos] == ';') or bytes[pos] == '}':
                if bytes[pos] == '}':
                    braces -= 1
                if braces == 0:
                    funcs.append(func)
                    in_func = False
            else:
                m = re.match(r'\b([a-zA-Z0-9_]+)\s*(?:::<.*?>\s*)?\(', bytes[pos:])
                if m:
                    line += bytes[pos:pos + m.span()[1]].count('\n')
                    pos += m.span()[1]
                    func.calls.append(FuncCall(m.group(1), Location(file, line)))
                    continue
            pos += 1
    return funcs


def check_funcs(funcs):
    for func in funcs:
        has_async = any([f.name.endswith("_async") for f in func.calls])
        has_wait = any([f.name == "wait_for" for f in func.calls])
        if (has_async or has_wait) and not func.name.endswith("_async"):
            print("Function %s calls an asynchronous function, but doesn't end with _async" % func)
        elif not (has_async or has_wait) and func.name.endswith("_async"):
            print("Function %s calls no asynchronous function, but ends with _async" % func)


def print_funcs(funcs):
    print("Parsed file %s:" % f)
    for func in funcs:
        print("  function %s:" % func)
        for call in func.calls:
            print("    calls %s" % call)


if len(sys.argv) < 2:
    exit("Usage: %s <dir>" % sys.argv[0])

for f in find_files(sys.argv[1], "*.rs"):
    funcs = parse_file(f)
    check_funcs(funcs)
