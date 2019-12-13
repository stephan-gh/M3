import os, sys

sys.path.append(os.path.realpath('hw/gem5/configs/example'))
from dtu_fs import *

options = getOptions()
root = createRoot(options)

cmd_list = options.cmd.split(",")

num_mem = 1
num_pes = int(os.environ.get('M3_GEM5_PES'))
fsimg = os.environ.get('M3_GEM5_FS')
fsimgnum = os.environ.get('M3_GEM5_FSNUM', '1')
dtupos = int(os.environ.get('M3_GEM5_DTUPOS', 0))
isa = os.environ.get('M3_ISA')
accs = ['indir', 'indir', 'indir', 'indir', 'copy', 'copy', 'copy', 'copy', 'rot13']
mem_pe = num_pes + len(accs)

pes = []

# create the core PEs
for i in range(0, num_pes):
    pe = createCorePE(noc=root.noc,
                      options=options,
                      no=i,
                      cmdline=cmd_list[i],
                      memPE=mem_pe,
                      # ARM only supports SPM for now
                      l1size=None if isa == 'arm' else '32kB',
                      l2size=None if isa == 'arm' else '256kB',
                      spmsize='32MB' if isa == 'arm' else None,
                      dtupos=dtupos)
    pes.append(pe)

options.cpu_clock = '1GHz'

# create accelerator PEs
for i in range(0, len(accs)):
    pe = createAccelPE(noc=root.noc,
                       options=options,
                       no=num_pes + i,
                       accel=accs[i],
                       memPE=mem_pe,
                       spmsize='2MB')
    pes.append(pe)

# create the memory PEs
for i in range(0, num_mem):
    pe = createMemPE(noc=root.noc,
                     options=options,
                     no=num_pes + len(accs) + i,
                     size='3072MB',
                     image=fsimg if i == 0 else None,
                     imageNum=int(fsimgnum))
    pes.append(pe)

runSimulation(root, options, pes)
