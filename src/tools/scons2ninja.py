#!/usr/bin/env python3
# -*- coding: utf-8 -*-

################################################################################
#
# scons2ninja: A script to create a Ninja build file from SCons.
#
# Copyright (c) 2019 Nils Asmussen
# Copyright (c) 2013 Remko Tronçon
#
# Permission is hereby granted, free of charge, to any person obtaining a copy of
# this software and associated documentation files (the "Software"), to deal in
# the Software without restriction, including without limitation the rights to
# use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
# the Software, and to permit persons to whom the Software is furnished to do so,
# subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
# FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
# COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
# IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
# CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
#
################################################################################

import re, os, os.path, subprocess, sys, fnmatch, shlex

################################################################################
# Helper methods & variables
################################################################################

SCRIPT = sys.argv[0]
if len(sys.argv) > 1 and sys.argv[1] == '--dir' :
  OUT_DIR = sys.argv[2]
  SCONS_ARGS = ' '.join(sys.argv[3:])
  OWN_ARGS = ' '.join(sys.argv[1:3])
else :
  OUT_DIR = "."
  SCONS_ARGS = ' '.join(sys.argv[1:])
  OWN_ARGS = ''

# TODO: Make this a tool-specific map
BINARY_FLAGS = ["-framework", "-arch", "-x", "--output-format", "-isystem", "-include"]

if sys.platform == 'win32' :
  LIB_PREFIX = ""
  LIB_SUFFIX = ""
  EXE_SUFFIX = ".exe"
else :
  LIB_PREFIX = "lib"
  LIB_SUFFIX = ".a"
  EXE_SUFFIX = ""

def is_regexp(x) :
  return 'match' in dir(x)

def is_list(l) :
  return type(l) is list

def escape(s) :
  return s.replace(' ', '$ ').replace(':', '$:')

def quote_spaces(s) :
  if ' ' in s :
    return '"' + s + '"'
  else :
    return s

def to_list(l) :
  if not l :
    return []
  if is_list(l) :
    return l
  return [l]

def partition(l, f) :
  x = []
  y = []
  for v in l :
    if f(v) :
      x.append(v)
    else :
      y.append(v)
  return (x, y)

def get_unary_flags(prefix, flags) :
  return [x[len(prefix):] for x in flags if x.lower().startswith(prefix.lower())]

def extract_unary_flags(prefix, flags) :
  f1, f2 = partition(flags, lambda x : x.lower().startswith(prefix.lower()))
  return ([f[len(prefix):] for f in f1], f2)

def extract_unary_flag(prefix, flags) :
  flag, flags = extract_unary_flags(prefix, flags)
  return (flag[0], flags)

def extract_binary_flag(prefix, flags) :
  i = flags.index(prefix)
  flag = flags[i + 1]
  del flags[i]
  del flags[i]
  return (flag, flags)

def get_non_flags(flags) :
  skip = False
  result = []
  for f in flags :
    if skip :
      skip = False
    elif f in BINARY_FLAGS :
      skip = True
    elif not f.startswith("/") and not f.startswith("-") :
      result.append(f)
  return result

def extract_non_flags(flags) :
  non_flags = get_non_flags(flags)
  return (non_flags, list(filter(lambda x : x not in non_flags, flags)))

def get_dependencies(target, build_targets) :
  result = []
  queue = list(dependencies.get(target, []))
  while len(queue) > 0 :
    n = queue.pop()
    # Filter out Value() results
    if n in build_targets or os.path.exists(n) :
      result.append(n)
      queue += list(dependencies.get(n, []))
  return result

def get_built_libs(libs, libpaths, outputs) :
  canonical_outputs = [os.path.abspath(p) for p in outputs]
  result = []
  for libpath in libpaths :
    for lib in libs :
      lib_libpath = os.path.join(libpath, LIB_PREFIX + lib + LIB_SUFFIX)
      if os.path.abspath(lib_libpath) in canonical_outputs :
        result.append(lib_libpath)
  return result

def parse_tool_command(line) :
  command = shlex.split(line)
  flags = command[1:]
  tool = os.path.splitext(os.path.basename(command[0]))[0]
  if tool.startswith('clang++') or tool.endswith('g++') :
    tool = "cxx"
  elif tool.startswith('clang') or tool.endswith('gcc') :
    tool = "cc"
  for t in ["cpp", "ar", "strip", "ranlib"]:
    if tool.endswith(t) :
      tool = t
      break
  if tool in ["cc", "cxx"] and not "-c" in flags :
    tool = "link"
  tool = tool.replace('-qt4', '')
  return tool, command, flags

def rglob(pattern, root = '.') :
  return [os.path.join(path, f) for path, dirs, files in os.walk(root) for f in fnmatch.filter(files, pattern)]

################################################################################
# Helper for building Ninja files
################################################################################

class NinjaBuilder :
  def __init__(self) :
    self._header = ""
    self.variables = ""
    self.rules = ""
    self._build = ""
    self.pools = ""
    self._flags = {}
    self.targets = []

  def header(self, text) :
    self._header += text + "\n"

  def rule(self, name, **kwargs) :
    self.rules += "rule " + name + "\n"
    for k, v in kwargs.items() :
      self.rules += "  " + str(k) + " = " + str(v) + "\n"
    self.rules += "\n"

  def pool(self, name, **kwargs) :
    self.pools += "pool " + name + "\n"
    for k, v in kwargs.items() :
      self.pools += "  " + str(k) + " = " + str(v) + "\n"
    self.pools += "\n"

  def variable(self, name, value) :
    self.variables += str(name) + " = " + str(value) + "\n"

  def build(self, target, rule, sources = None, **kwargs) :
    self._build += "build " + self.to_string(target) + ": " + rule
    if sources :
      self._build += " " + self.to_string(sources)
    if 'deps' in kwargs and kwargs['deps'] :
      self._build += " | " + self.to_string(kwargs["deps"])
    if 'order_deps' in kwargs :
      self._build += " || " + self.to_string(kwargs['order_deps'])
    self._build += "\n"
    for var, value in kwargs.items() :
      if var in ['deps', 'order_deps'] :
        continue
      value = self.to_string(value, quote = True)
      if var == 'cmd' :
        if tools[rule] == value :
          continue
        var = rule
      elif var.endswith("flags") :
        value = self.get_flags_variable(var, value)
      self._build += "  " + var + " = " + value + "\n"
    self.targets += to_list(target)

  def header_targets(self) :
    return [x for x in self.targets if x.endswith('.h') or x.endswith('.hh')]

  def serialize(self) :
    result = ""
    result += self._header + "\n"
    result += self.variables + "\n"
    for prefix in self._flags.values() :
      for k, v in prefix.items() :
        result += v + " = " + k + "\n"
    result += "\n"
    result += self.pools + "\n"
    result += self.rules + "\n"
    result += self._build + "\n"
    return result

  def to_string(self, lst, quote = False) :
    if is_list(lst) :
      if quote :
        return ' '.join([quote_spaces(x) for x in lst])
      else :
        return ' '.join([escape(x) for x in lst])
    if is_regexp(lst) :
      return ' '.join([escape(x) for x in self.targets if lst.match(x)])
    return escape(lst)

  def get_flags_variable(self, flags_type, flags) :
    if len(flags) == 0 :
      return ''
    if flags_type not in self._flags :
      self._flags[flags_type] = {}
    type_flags = self._flags[flags_type]
    if flags not in type_flags :
      type_flags[flags] = flags_type + "_" + str(len(type_flags))
    return "$" + type_flags[flags]


################################################################################
# Configuration
################################################################################

ninja_post = None
scons_cmd = "scons"
scons_dependencies = ['SConstruct'] + rglob('SConscript')

def ninja_custom_command(ninja, line) :
  return False

CONFIGURATION_FILE = '.scons2ninja.conf'
exec(open(CONFIGURATION_FILE).read())

scons_dependencies = [os.path.normpath(x) for x in scons_dependencies]


################################################################################
# Rules
################################################################################

ninja = NinjaBuilder()

ninja.pool('scons_pool', depth = 1)

if sys.platform == 'win32' :
  ninja.rule('cl',
    deps = 'msvc',
    command = '$cl /showIncludes $clflags -c $in /Fo$out',
    description = 'CXX $out')

  ninja.rule('link',
    command = '$link $in $linkflags $libs /out:$out',
    description = 'LINK $out')

  ninja.rule('link_mt',
    command = '$link $in $linkflags $libs /out:$out ; $mt $mtflags',
    description = 'LINK $out')

  ninja.rule('lib',
    command = '$lib $libflags /out:$out $in',
    description = 'AR $out')

  ninja.rule('rc',
    command = '$rc $rcflags /Fo$out $in',
    description = 'RC $out')

  # SCons doesn't touch files if they didn't change, which makes
  # ninja rebuild the file over and over again. There's no touch on Windows :(
  # Could implement it with a script, but for now, delete the file if
  # this problem occurs. I'll fix it if it occurs too much.
  ninja.rule('scons',
    command = scons_cmd + " ${scons_args} $out",
    pool = 'scons_pool',
    description = 'GEN $out')

  ninja.rule('install', command = 'cmd /c copy $in $out')
  ninja.rule('run', command = '$in')
else :
  ninja.rule('cxx',
    deps = 'gcc',
    depfile = '$out.d',
    command = '$cxx -MD -MF $out.d $cxxflags -c $in -o $out',
    description = 'CXX $out')

  ninja.rule('cc',
    deps = 'gcc',
    depfile = '$out.d',
    command = '$cc -MD -MF $out.d $ccflags -c $in -o $out',
    description = 'CC $out')

  ninja.rule('link',
    command = '$link -o $out $in $linkflags',
    description = 'LINK $out')

  ninja.rule('strip',
    command = '$strip $stripflags -o $out $in',
    description = 'STRIP $out')

  ninja.rule('cpp',
    command = '$cpp $cppflags $in $out',
    description = 'CPP $out')

  ninja.rule('ar',
    command = '$ar $arflags $out $in && ranlib $out',
    description = 'AR $out')

  ninja.rule('ln',
    command = '$ln $lnflags $$(readlink -f $in) $out',
    description = 'LN $out')

  # SCons doesn't touch files if they didn't change, which makes
  # ninja rebuild the file over and over again. Touching solves this.
  ninja.rule('scons',
    command = scons_cmd + " $out && touch $out",
    pool = 'scons_pool',
    description = 'GEN $out')

  ninja.rule('install', command = 'install $in $out')
  ninja.rule('run', command = './$in')


ninja.rule('moc',
  command = '$moc $mocflags -o $out $in',
  description = 'MOC $out')

ninja.rule('rcc',
  command = '$rcc $rccflags -name $name -o $out $in',
  description = 'RCC $out')

ninja.rule('uic',
  command = '$uic $uicflags -o $out $in',
  description = 'UIC $out')

ninja.rule('lrelease',
  command = '$lrelease $lreleaseflags $in -qm $out',
  description = 'LRELEASE $out')

ninja.rule('ibtool',
  command = '$ibtool $ibtoolflags --compile $out $in',
  description = 'IBTOOL $out')

ninja.rule('dsymutil',
  command = '$dsymutil $dsymutilflags -o $out $in',
  description = 'DSYMUTIL $out')

ninja.rule('generator',
  command = "python " + SCRIPT + " ${own_args} ${scons_args}",
  depfile = OUT_DIR + "/.scons2ninja.deps",
  pool = 'scons_pool',
  generator = '1',
  description = 'Regenerating build.ninja')


################################################################################
# Build Statements
################################################################################

scons_generate_cmd = scons_cmd + " " + SCONS_ARGS + " --tree=all,prune dump_trace=1"
#scons_generate_cmd = 'cmd /c type scons2ninja.in'
#scons_generate_cmd = 'cat scons2ninja.in'

# Pass 1: Parse dependencies (and prefilter some build rules)
build_lines = []
dependencies = {}
mtflags = {}
previous_file = None
f = subprocess.Popen(scons_generate_cmd, stdout = subprocess.PIPE, stderr = subprocess.PIPE, shell=True)
stage = 'preamble'
skip_nth_line = -1
stack = ['.']
for line in f.stdout :
  line = line.rstrip().decode()

  # Skip lines if requested from previous command
  if skip_nth_line >= 0 :
    skip_nth_line -= 1
  if skip_nth_line == 0 :
    continue

  if line.startswith('scons: done building targets') :
    break

  if stage == "preamble" :
    # Pass all lines from the SCons configuration step to output
    if line.startswith("scons: Building targets ...") :
      stage = "build"
    else :
      print(line)
  elif stage == "build" :
    if line.startswith('+-') :
      stage = "dependencies"
    elif re.match("^Using tempfile", line) :
      # Ignore response files from MSVS
      skip_nth_line = 2
    elif not line.startswith("scons: building associated VariantDir targets:") :
      build_lines.append(line)

      # Already detect targets that will need 'mt'
      tool, _, flags = parse_tool_command(line)
      if tool == 'mt' :
        target = get_unary_flags("-outputresource:", flags)[0]
        target = target[0:target.index(';')]
        mtflags[target] = flags

  elif stage == "dependencies" :
    if not re.match('^[\s|]+\+\-', line) :
      # Work around bug in SCons that splits output over multiple lines
      continue

    level = int(line.index('+-') / 2)
    filename = line[level*2+2:]
    if filename.startswith('[') :
      filename = filename[1:-1]

    # Check if we use the 'fixed' format which escapes filenamenames
    if filename.startswith('\'') and filename.endswith('\'') :
      filename = eval(filename)

    if level < len(stack) :
      stack = stack[0:level]
    elif level > len(stack) :
      if level != len(stack) + 1 :
        raise Exception("Internal Error" )
      stack.append(previous_filename)

    # Skip absolute paths
    if not os.path.isabs(filename) :
      target = stack[-1]
      if target not in dependencies :
        dependencies[target] = []
      dependencies[target].append(filename)
    previous_filename = filename

if f.wait() != 0 :
  print("Error calling '" + scons_generate_cmd + "'")
  print(f.stderr.read())
  exit(-1)

# Pass 2: Parse build rules
tools = {}
for line in build_lines :
  # Custom python function
  m = re.match('^(\w+)\(\[([^\]]*)\]', line)
  if m :
    out = [x[1:-1] for x in m.group(2).split(',')]
    for x in out :
      # 'Note' = To be more correct, deps should also include $scons_dependencies,
      # but this regenerates a bit too often, so leaving it out for now.
      ninja.build(x, 'scons', None, deps = sorted(get_dependencies(x, ninja.targets)))
    continue


  # TextFile
  m = re.match("^Creating '([^']+)'", line)
  if m :
    out = m.group(1)
    # Note: To be more correct, deps should also include $scons_dependencies,
    # but this regenerates a bit too often, so leaving it out for now.
    ninja.build(out, 'scons', None, deps = sorted(get_dependencies(out, ninja.targets)))
    continue

  # Install
  m = re.match('^Install file: "(.*)" as "(.*)"', line)
  if m :
    ninja.build(m.group(2), 'install', m.group(1))
    continue

  m = re.match('^Install directory: "(.*)" as "(.*)"', line)
  if m :
    for source in rglob('*', m.group(1)) :
      if os.path.isdir(source) :
        continue
      target = os.path.join(m.group(2), os.path.relpath(source, m.group(1)))
      ninja.build(target, 'install', source)
    continue

  # Tools
  tool, command, flags = parse_tool_command(line)
  if not tool in tools:
    tools[tool] = command[0]

  ############################################################
  # clang/gcc tools
  ############################################################

  if tool == 'cc':
    out, flags = extract_binary_flag("-o", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'cc', files, order_deps = '_generated_headers', ccflags = flags, cmd = command[0])

  elif tool == 'cxx':
    out, flags = extract_binary_flag("-o", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'cxx', files, order_deps = '_generated_headers', cxxflags = flags, cmd = command[0])

  elif tool == 'link':
    out, flags = extract_binary_flag("-o", flags)
    files, flags = extract_non_flags(flags)
    libs = get_unary_flags('-l', flags)
    libpaths = get_unary_flags("-L", flags)
    deps = get_built_libs(libs, libpaths, ninja.targets)
    deps += get_unary_flags('-Wl,-T,', flags)
    ninja.build(out, 'link', files, deps = sorted(deps), linkflags = flags, cmd = command[0])

  elif tool == 'ar':
    objects, flags = partition(flags, lambda x: x.endswith('.o'))
    libs, flags = partition(flags, lambda x: x.endswith('.a'))
    out = libs[0]
    ninja.build(out, 'ar', objects, arflags = flags, cmd = command[0])

  elif tool == 'strip':
    out, flags = extract_binary_flag("-o", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'strip', files, stripflags = flags, cmd = command[0])

  elif tool == 'cpp':
    out, flags = extract_binary_flag("-o", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'cpp', files, cppflags = flags, cmd = command[0])

  elif tool == 'ln':
    files = [flags[len(flags) - 2].replace(os.environ['PWD'] + '/', '')]
    out = flags[len(flags) - 1]
    del flags[len(flags) - 2]
    del flags[len(flags) - 1]
    ninja.build(out, 'ln', files, lnflags = flags, cmd = command[0])

  elif tool == 'ranlib':
    pass


  ############################################################
  # MSVC tools
  ############################################################

  elif tool == 'cl':
    out, flags = extract_unary_flag("/Fo", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'cl', files, order_deps = '_generated_headers', clflags = flags)

  elif tool == 'lib':
    out, flags = extract_unary_flag("/out:", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'lib', files, libflags = flags)

  elif tool == 'link':
    objects, flags = partition(flags, lambda x: x.endswith('.obj') or x.endswith('.res'))
    out, flags = extract_unary_flag("/out:", flags)
    libs, flags = partition(flags, lambda x: not x.startswith("/") and x.endswith(".lib"))
    libpaths = get_unary_flags("/libpath:", flags)
    deps = get_built_libs(libs, libpaths, ninja.targets)
    if out in mtflags :
      ninja.build(out, 'link_mt', objects, deps = sorted(deps),
        libs = libs, linkflags = flags, mtflags = mtflags[out])
    else :
      ninja.build(out, 'link', objects, deps = sorted(deps),
        libs = libs, linkflags = flags)

  elif tool == 'rc':
    out, flags = extract_unary_flag("/fo", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'rc', files[0], order_deps = '_generated_headers', rcflags = flags)

  elif tool == 'mt':
    # Already handled
    pass

  ############################################################
  # Qt tools
  ############################################################

  elif tool == 'moc':
    out, flags = extract_binary_flag("-o", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'moc', files, mocflags = flags)

  elif tool == 'uic':
    out, flags = extract_binary_flag("-o", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'uic', files, uicflags = flags)

  elif tool == 'lrelease':
    out, flags = extract_binary_flag("-qm", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'lrelease', files, lreleaseflags = flags)

  elif tool == 'rcc':
    out, flags = extract_binary_flag("-o", flags)
    name, flags = extract_binary_flag("-name", flags)
    files, flags = extract_non_flags(flags)
    deps = list(set(get_dependencies(out, ninja.targets)) - set(files))
    ninja.build(out, 'rcc', files, deps = sorted(deps), name = name, rccflags = flags)

  ############################################################
  # OS X tools
  ############################################################

  elif tool == 'ibtool':
    out, flags = extract_binary_flag("--compile", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'ibtool', files, ibtoolflags = flags)

  elif tool == 'dsymutil':
    out, flags = extract_binary_flag("-o", flags)
    files, flags = extract_non_flags(flags)
    ninja.build(out, 'dsymutil', files, dsymutilflags = flags)

  elif not ninja_custom_command(ninja, line)  :
    raise Exception("Unknown tool: '" + line + "'")


# Phony target for all generated headers, used as an order-only depency from all C/C++ sources
ninja.build('_generated_headers', 'phony', ninja.header_targets())

# Regenerate build.ninja file
ninja.build(OUT_DIR + '/build.ninja', 'generator', [], deps = [SCRIPT, CONFIGURATION_FILE])

# Header & variables
ninja.header("# This file is generated by " + SCRIPT)
ninja.variable("ninja_required_version", "1.3")
ninja.variable("own_args", OWN_ARGS)
ninja.variable("scons_args", SCONS_ARGS)
for k, v in tools.items() :
  ninja.variable(k, v)

# Extra customizations
if not ninja_post is None:
  ninja_post(ninja)


################################################################################
# Result
################################################################################

f = open(OUT_DIR + "/.scons2ninja.deps", "w")
f.write(OUT_DIR + "/build.ninja: " + " ".join([d for d in scons_dependencies if os.path.exists(d)]) + "\n")
f.close()

f = open(OUT_DIR + "/build.ninja", "w")
f.write(ninja.serialize())
f.close()
