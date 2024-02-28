import os
import sys

sys.path.append(os.path.realpath('platform/gem5/configs/example'))  # NOQA
from tcu_fs import *

DEFAULT_O3_CPU = os.environ.get('M3_GEM5_O3_CPU', 'DerivO3CPU')
DEFAULT_MINOR_CPU = os.environ.get('M3_GEM5_MINOR_CPU', 'MinorCPU')

options = getOptions()
root = createRoot(options)

# cmd is only used to extract the kernel cmdline
cmd_list = options.cmd.split(",")
kernel_cmdline = cmd_list[0]
assert all(cmd == '' for cmd in cmd_list[1:]), "cmdline must be empty for all except the first kernel tile"

rot_layers = options.rot_layers.split(",")
assert rot_layers[0], "must specify at least one RoT layer"

num_eps = 192 if os.environ.get('M3_TARGET') == 'gem5' else 128
num_mem = 1
num_flash = 1
num_tiles = int(os.environ.get('M3_GEM5_TILES'))
num_o3 = int(os.environ.get('M3_GEM5_O3_CORES', num_tiles))
num_minor = int(os.environ.get('M3_GEM5_MINOR_CORES', num_tiles))
num_kecacc = int(os.environ.get('M3_GEM5_KECACC_TILES', 1))

hard_disk0 = os.environ.get('M3_GEM5_IDE_DRIVE')

mem_tile = None
mem_tile_id = None
flash_tile = None
flash_tile_id = None

tiles = []

# create the memory tiles
for i in range(0, num_mem):
    tile_id = TileId(0, len(tiles))
    tile = createMemTile(noc=root.noc,
                         options=options,
                         id=tile_id,
                         size='3GB',
                         epCount=num_eps)
    tiles.append(tile)
    mem_tile = mem_tile or tile
    mem_tile_id = mem_tile_id or tile_id

# create the flash tiles
for i in range(0, num_flash):
    tile_id = TileId(0, len(tiles))
    tile = createMemTile(noc=root.noc,
                         options=options,
                         id=tile_id,
                         size=os.environ.get('M3_GEM5_FLASH_SIZE', '512MB'),
                         memType='Flash',
                         epCount=num_eps)
    tiles.append(tile)
    flash_tile = flash_tile or tile
    flash_tile_id = flash_tile_id or tile_id

# create the in-order core tiles
options.cpu_type = DEFAULT_MINOR_CPU
for i in range(0, num_minor):
    tile = createCoreTile(noc=root.noc,
                          options=options,
                          id=TileId(0, len(tiles)),
                          cmdline='',
                          memTile=None,
                          l1size='32kB',
                          l2size='128kB',
                          epCount=num_eps)
    tiles.append(tile)

# create the out-of-order core tiles
options.cpu_type = DEFAULT_O3_CPU
for i in range(0, num_o3):
    tile = createCoreTile(noc=root.noc,
                          options=options,
                          id=TileId(0, len(tiles)),
                          cmdline='',
                          memTile=None,
                          l1size='32kB',
                          l2size='256kB',
                          epCount=num_eps)
    tiles.append(tile)

options.cpu_type = DEFAULT_MINOR_CPU
for i in range(0, num_kecacc):
    tile = createKecAccTile(noc=root.noc,
                            options=options,
                            id=TileId(0, len(tiles)),
                            cmdline='',
                            memTile=None,
                            spmsize='64MB',
                            epCount=num_eps)
    tiles.append(tile)

old_cpu_options = options.cpu_type, options.cpu_clock
options.cpu_type = os.environ.get('M3_GEM5_ROT_CPU', DEFAULT_MINOR_CPU)
options.cpu_clock = os.environ.get('M3_GEM5_ROT_CPUFREQ', '100MHz')
tile = createRoTTile(noc=root.noc,
                     options=options,
                     id=TileId(0, len(tiles)),
                     cmdline=rot_layers[0],
                     rotLayers=rot_layers[1:],
                     kernelCmdline=kernel_cmdline,
                     flashTile=flash_tile,
                     # flashTile=mem_tile,  # RoT can also run fetch data directly from memory tile
                     # FIXME: The RoT boot layers run with 256 KiB but TileMux/RoTS currently need more
                     spmsize=os.environ.get('M3_GEM5_ROT_MEMSIZE', '64MB'),
                     epCount=num_eps)
tiles.append(tile)
options.cpu_type, options.cpu_clock = old_cpu_options

# create the persistent storage tile
if hard_disk0 and os.path.isfile(hard_disk0):
    tile = createStorageTile(noc=root.noc,
                             options=options,
                             id=TileId(0, len(tiles)),
                             memTile=None,
                             img0=hard_disk0,
                             epCount=num_eps)
    tiles.append(tile)#
else:
    print('NOTE: Skipping persistent storage tile because no hard disk image was specified')
    print()

# create ether tile connected to the host
if os.path.isdir("/sys/class/net/gem5-tap"):
    ether = createEtherTile(noc=root.noc,
                             options=options,
                             id=TileId(0, len(tiles)),
                             memTile=None,
                             epCount=num_eps)
    tiles.append(ether)
    ether.ethertap = EtherTap(tap=ether.nic.interface)
else:
    print('NOTE: Skipping ether tile because no gem5-tap interface was found')
    print('Use the following commands to set it up:')
    print('\t$ sudo ip tuntap add gem5-tap mode tap')
    print('\t$ sudo ip link set gem5-tap up')
    print('\t$ sudo ip addr add 192.168.42.2/24 dev gem5-tap')
    print()

# create tile for serial input
tile = createSerialTile(noc=root.noc,
                        options=options,
                        id=TileId(0, len(tiles)),
                        memTile=None,
                        epCount=num_eps)
tiles.append(tile)

runSimulation(root, options, tiles)
