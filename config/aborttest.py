import os, sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))
from tcu_fs import *

options = getOptions()
root = createRoot(options)

num_eps = 128 if os.environ.get('M3_TARGET') == 'hw' else 192
num_tiles = 1
mem_tile = num_tiles
tiles = []

for i in range(0, num_tiles):
    tile = createAbortTestTile(noc=root.noc,
                               options=options,
                               no=i,
                               memTile=mem_tile,
                               spmsize='32MB',
                               epCount=num_eps)
    # use 64 bytes as the block size here to test whether it works with multiple memory accesses
    tile.tcu.block_size = "64B"
    tiles.append(tile)

tile = createMemTile(noc=root.noc,
                     options=options,
                     no=num_tiles,
                     size='3072MB',
                     epCount=num_eps)

tiles.append(tile)

# this is required in order to not occupy the noc xbar for a
# longer amount of time as we need to handle the request on the remote side
root.noc.width = 64

runSimulation(root, options, tiles)
