import os
import sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))
from tcu_fs import *  # NOQA

options = getOptions()
options.initrd = 'build/cross-riscv/images/rootfs.cpio'
options.kernel = 'build/riscv-pk/gem5/bbl'
options.command_line = 'earlycon=sbi console=ttyS0 root=/dev/initrd'
root = createRoot(options)

cmd_list = options.cmd.split(",")

num_eps = 192 if os.environ.get('M3_TARGET') == 'gem5' else 128
num_tiles = int(os.environ.get('M3_GEM5_TILES'))
mem_tile = TileId(0, num_tiles)

tiles = []

# create the core tiles
for i in range(0, num_tiles - 1):
    tile = createCoreTile(noc=root.noc,
                          options=options,
                          id=TileId(0, i),
                          cmdline=cmd_list[i],
                          memTile=mem_tile if cmd_list[i] != "" else None,
                          l1size='32kB',
                          l2size='256kB',
                          epCount=num_eps)
    tiles.append(tile)

# create the tile for Linux
tile = createLinuxTile(noc=root.noc,
                       options=options,
                       id=TileId(0, num_tiles - 1),
                       memTile=None,
                       l1size='32kB',
                       l2size='256kB',
                       epCount=num_eps)
tiles.append(tile)

# create the memory tile
memory_tile = createMemTile(noc=root.noc,
                            options=options,
                            id=mem_tile,
                            size='3072MB',
                            epCount=num_eps)
tiles.append(memory_tile)

# create tile for serial input
tile = createSerialTile(noc=root.noc,
                        options=options,
                        id=TileId(0, num_tiles + 1),
                        memTile=None,
                        epCount=num_eps)
tiles.append(tile)

runSimulation(root, options, tiles)
