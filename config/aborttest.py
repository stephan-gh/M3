import os, sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))
from tcu_fs import *

options = getOptions()
root = createRoot(options)

num_eps = 64 if os.environ.get('M3_TARGET') == 'hw' else 192
num_pes = 1
mem_pe = num_pes
pes = []

for i in range(0, num_pes):
    pe = createAbortTestPE(noc=root.noc,
                           options=options,
                           no=i,
                           memPE=mem_pe,
                           spmsize='32MB',
                           epCount=num_eps)
    # use 64 bytes as the block size here to test whether it works with multiple memory accesses
    pe.tcu.block_size = "64B"
    pes.append(pe)

pe = createMemPE(noc=root.noc,
                 options=options,
                 no=num_pes,
                 size='3072MB',
                 epCount=num_eps)

pes.append(pe)

# this is required in order to not occupy the noc xbar for a
# longer amount of time as we need to handle the request on the remote side
root.noc.width = 64

runSimulation(root, options, pes)
