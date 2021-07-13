import os, sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))
from tcu_fs import *

options = getOptions()
root = createRoot(options)

cmd_list = options.cmd.split(",")

num_eps = 128 if os.environ.get('M3_TARGET') == 'hw' else 192
num_mem = 1
num_pes = int(os.environ.get('M3_GEM5_PES'))
num_kecacc = 1
fsimg = os.environ.get('M3_GEM5_FS')
fsimgnum = os.environ.get('M3_GEM5_FSNUM', '1')
mem_pe = num_pes + num_kecacc + 1

pes = []

# create the core PEs
for i in range(0, num_pes):
    pe = createCorePE(noc=root.noc,
                      options=options,
                      no=i,
                      cmdline=cmd_list[i],
                      memPE=mem_pe,
                      spmsize='32MB',
                      epCount=num_eps)
    pes.append(pe)

for i in range(0, num_kecacc):
    pe = createKecAccPE(noc=root.noc,
                        options=options,
                        no=num_pes + i,
                        cmdline=cmd_list[1],  # FIXME
                        memPE=mem_pe,
                        spmsize='32MB',
                        epCount=num_eps)
    pes.append(pe)

# create PE for serial input
pe = createSerialPE(noc=root.noc,
                    options=options,
                    no=num_pes + num_kecacc,
                    memPE=mem_pe,
                    epCount=num_eps)
pes.append(pe)

# create the memory PEs
for i in range(0, num_mem):
    pe = createMemPE(noc=root.noc,
                     options=options,
                     no=num_pes + num_kecacc + 1 + i,
                     size='3072MB',
                     image=fsimg if i == 0 else None,
                     imageNum=int(fsimgnum),
                     epCount=num_eps)
    pes.append(pe)

runSimulation(root, options, pes)
