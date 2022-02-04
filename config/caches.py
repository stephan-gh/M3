import os, sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))
from tcu_fs import *

options = getOptions()
root = createRoot(options)

cmd_list = options.cmd.split(",")

num_eps = 128 if os.environ.get('M3_TARGET') == 'hw' else 192
num_mem = 1
num_tiles = int(os.environ.get('M3_GEM5_TILES'))
fsimg = os.environ.get('M3_GEM5_FS')
fsimgnum = os.environ.get('M3_GEM5_FSNUM', '1')
tcupos = int(os.environ.get('M3_GEM5_TCUPOS', 0))
mem_tile = num_tiles + 1

# Memory watch example:
# options.mem_watches = {
#     5 : [
#         AddrRange(0x0, 0x100000),
#         AddrRange(0xf0000000, 0xf0001000),
#     ],
# }

tiles = []

# create the core tiles
for i in range(0, num_tiles):
    tile = createCoreTile(noc=root.noc,
                          options=options,
                          no=i,
                          cmdline=cmd_list[i],
                          memTile=mem_tile,
                          l1size='32kB',
                          l2size='256kB',
                          tcupos=tcupos,
                          epCount=num_eps)
    tiles.append(tile)

# create tile for serial input
tile = createSerialTile(noc=root.noc,
                        options=options,
                        no=num_tiles,
                        memTile=mem_tile,
                        epCount=num_eps)
tiles.append(tile)

# create the memory tiles
for i in range(0, num_mem):
    tile = createMemTile(noc=root.noc,
                         options=options,
                         no=num_tiles + 1 + i,
                         size='3072MB',
                         image=fsimg if i == 0 else None,
                         imageNum=int(fsimgnum),
                         epCount=num_eps)
    tiles.append(tile)

runSimulation(root, options, tiles)
