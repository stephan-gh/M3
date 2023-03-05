import os, sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))
from tcu_fs import *

options = getOptions()
root = createRoot(options)

cmd_list = options.cmd.split(",")

num_eps = 192 if os.environ.get('M3_TARGET') == 'gem5' else 128
num_mem = 1
num_tiles = int(os.environ.get('M3_GEM5_TILES'))
mem_tile = TileId(0, num_tiles)

tiles = []

# create the core tiles
for i in range(0, num_tiles):
    tile = createCoreTile(noc=root.noc,
                          options=options,
                          id=TileId(0, i),
                          cmdline=cmd_list[i],
                          memTile=mem_tile,
                          spmsize='64MB',
                          epCount=num_eps)
    tiles.append(tile)

# create the memory tiles
for i in range(0, num_mem):
    tile = createMemTile(noc=root.noc,
                         options=options,
                         id=TileId(0, num_tiles + i),
                         size='3072MB',
                         epCount=num_eps)
    tiles.append(tile)

# create tile for serial input
tile = createSerialTile(noc=root.noc,
                        options=options,
                        id=TileId(0, num_tiles + num_mem),
                        memTile=mem_tile,
                        epCount=num_eps)
tiles.append(tile)

runSimulation(root, options, tiles)
