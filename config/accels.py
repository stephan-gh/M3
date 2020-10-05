import os, sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))
from tcu_fs import *

options = getOptions()
root = createRoot(options)

cmd_list = options.cmd.split(",")

num_eps = 64 if os.environ.get('M3_TARGET') == 'hw' else 192
num_mem = 1
num_pes = int(os.environ.get('M3_GEM5_PES'))
fsimg = os.environ.get('M3_GEM5_FS')
fsimgnum = os.environ.get('M3_GEM5_FSNUM', '1')
tcupos = int(os.environ.get('M3_GEM5_TCUPOS', 0))
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
                      l1size='32kB',
                      l2size='256kB',
                      tcupos=tcupos,
                      epCount=num_eps)
    pes.append(pe)

options.cpu_clock = '1GHz'

# create accelerator PEs
for i in range(0, len(accs)):
    pe = createAccelPE(noc=root.noc,
                       options=options,
                       no=num_pes + i,
                       accel=accs[i],
                       memPE=mem_pe,
                       spmsize='4MB',
                       epCount=num_eps)
    pes.append(pe)

# create the memory PEs
for i in range(0, num_mem):
    pe = createMemPE(noc=root.noc,
                     options=options,
                     no=num_pes + len(accs) + i,
                     size='3072MB',
                     image=fsimg if i == 0 else None,
                     imageNum=int(fsimgnum),
                     epCount=num_eps)
    pes.append(pe)

runSimulation(root, options, pes)
