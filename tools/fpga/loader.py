import os
import sys

from elftools.elf.elffile import ELFFile

from tcu import EP, MemEP, Flags

import utils

DRAM_SIZE = 2 * 1024 * 1024 * 1024
DRAM_OFF = 0x10000000
ENV = 0x10001000
MEM_TILE = 8

KENV_ADDR = 0
KENV_SIZE = 4 * 1024
SERIAL_ADDR = KENV_ADDR + KENV_SIZE
SERIAL_SIZE = 4 * 1024
PMP_ADDR = SERIAL_ADDR + SERIAL_SIZE


class Loader:
    def __init__(self, pmp_size):
        self.pmp_size = pmp_size

    def init(self, tiles, dram, kernels, mods, logflags, vm):
        # load boot info into DRAM
        kernel_tiles = kernels[0:len(tiles)]
        if vm:
            mods_addr = PMP_ADDR + (len(kernel_tiles) * self.pmp_size)
        else:
            mods_addr = PMP_ADDR + (len(tiles) * self.pmp_size)
        self._load_boot_info(dram, mods, tiles, vm, mods_addr)

        # init all tiles
        for i, tile in enumerate(tiles, 0):
            self._init_tile(dram, tile, i, i < len(kernel_tiles), vm)

        # load kernels on tiles
        for i, pargs in enumerate(kernel_tiles, 0):
            self._load_prog(dram, tiles, i, pargs.split(' '), vm, logflags)

    def start(self, tiles, debug):
        # start kernel tiles
        debug_tile = len(tiles) if debug is None else debug
        for i, tile in enumerate(tiles, 0):
            if i != debug_tile:
                # start core (via interrupt 0)
                tiles[i].rocket_start()

    def _init_tile(self, dram, tile, i, loaded, vm):
        # reset TCU (clear command log and reset registers except FEATURES and EPs)
        tile.tcu_reset()

        # enable instruction trace for all tiles (doesn't cost anything)
        tile.rocket_enableTrace()

        # set features: privileged, vm, ctxsw
        tile.tcu_set_features(1, vm, 1)

        # invalidate all EPs
        for ep in range(0, 127):
            tile.tcu_set_ep(ep, EP.invalid())

        # init PMP EP (for loaded tiles or if SPM should be emulated)
        if loaded or not vm:
            mem_begin = PMP_ADDR + i * self.pmp_size
            mem_size = self.pmp_size

            # install first PMP EP
            pmp_ep = MemEP()
            pmp_ep.set_chip(dram.mem.nocid[0])
            pmp_ep.set_tile(dram.mem.nocid[1])
            pmp_ep.set_act(0xFFFF)
            pmp_ep.set_flags(Flags.READ | Flags.WRITE)
            pmp_ep.set_addr(mem_begin)
            pmp_ep.set_size(mem_size)
            tile.tcu_set_ep(0, pmp_ep)

    def _load_boot_info(self, dram, mods, tiles, vm, mods_addr):
        # boot info
        kenv_off = KENV_ADDR
        utils.write_u64(dram, kenv_off + 0 * 8, len(mods))    # mod_count
        utils.write_u64(dram, kenv_off + 1 * 8, len(tiles) + 1)  # tile_count
        utils.write_u64(dram, kenv_off + 2 * 8, 1)            # mem_count
        utils.write_u64(dram, kenv_off + 3 * 8, 0)            # serv_count
        kenv_off += 8 * 4

        # mods
        for m in mods:
            mod_size = self._add_mod(dram, mods_addr, m, kenv_off)
            mods_addr = (mods_addr + mod_size + 4096 - 1) & ~(4096 - 1)
            kenv_off += 80

        # tile descriptors
        for x in range(0, len(tiles)):
            utils.write_u64(dram, kenv_off, self._tile_desc(tiles, x, vm))         # PM
            kenv_off += 8
        utils.write_u64(dram, kenv_off, self._tile_desc(tiles, len(tiles), False))  # dram1
        kenv_off += 8

        # mems
        mem_start = mods_addr
        utils.write_u64(dram, kenv_off + 0, utils.glob_addr(MEM_TILE, mem_start))  # addr
        utils.write_u64(dram, kenv_off + 8, DRAM_SIZE - mem_start)          # size

    def _load_prog(self, dram, tiles, i, args, vm, logflags):
        pm = tiles[i]

        # start core
        pm.start()

        print("%s: loading %s..." % (pm.name, args[0]))
        sys.stdout.flush()

        # verify entrypoint, because inject a jump instruction below that jumps to that address
        with open(args[0], 'rb') as f:
            elf = ELFFile(f)
            if elf.header['e_entry'] != 0x10003000:
                sys.exit("error: {} has entry {:#x}, not 0x10003000.".format(
                    args[0], elf.header['e_entry']))

        mem_begin = PMP_ADDR + i * self.pmp_size

        # load ELF file
        dram.mem.write_elf(args[0], mem_begin - DRAM_OFF)
        sys.stdout.flush()

        desc = self._tile_desc(tiles, i, vm)
        kenv = utils.glob_addr(MEM_TILE, KENV_ADDR) if i == 0 else 0

        # write arguments and env vars
        argv = ENV + 0x400
        envp = self._write_args(dram, args, argv, mem_begin)
        if logflags:
            self._write_args(dram, ["LOG=" + logflags], envp, mem_begin)
        else:
            envp = 0

        # init environment
        dram_env = ENV + mem_begin - DRAM_OFF
        utils.write_u64(dram, dram_env - 0x1000, 0x0000306f)  # j _start (+0x3000)
        utils.write_u64(dram, dram_env + 0, 1)           # platform = HW
        utils.write_u64(dram, dram_env + 8, i)           # chip, tile
        utils.write_u64(dram, dram_env + 16, desc)       # tile_desc
        utils.write_u64(dram, dram_env + 24, len(args))  # argc
        utils.write_u64(dram, dram_env + 32, argv)       # argv
        utils.write_u64(dram, dram_env + 40, envp)       # envp
        utils.write_u64(dram, dram_env + 48, kenv)       # kenv
        utils.write_u64(dram, dram_env + 56, len(tiles) + 1)  # raw tile count
        # tile ids
        env_off = 64
        for tile in tiles:
            utils.write_u64(dram, dram_env + env_off, tile.nocid[0] << 8 | tile.nocid[1])
            env_off += 8
        utils.write_u64(dram, dram_env + env_off, dram.mem.nocid[0] << 8 | dram.mem.nocid[1])

        sys.stdout.flush()

    def _add_mod(self, dram, addr, mod, offset):
        (name, path) = mod.split('=')
        path = os.path.basename(path)
        size = os.path.getsize(path)
        utils.write_u64(dram, offset + 0x0, utils.glob_addr(MEM_TILE, addr))
        utils.write_u64(dram, offset + 0x8, size)
        utils.write_str(dram, name, offset + 16)
        self._write_file(dram, path, addr)
        return size

    def _write_file(self, mod, file, offset):
        print("%s: loading %s with %u bytes to %#x" %
              (mod.name, file, os.path.getsize(file), offset))
        sys.stdout.flush()

        with open(file, "rb") as f:
            content = f.read()
        mod.mem.write_bytes_checked(offset, content, True)

    def _write_args(self, dram, args, argv, mem_begin):
        argc = len(args)
        args_addr = argv + (argc + 1) * 8
        for (idx, a) in enumerate(args, 0):
            # write pointer
            utils.write_u64(dram, argv + (mem_begin - DRAM_OFF) + idx * 8, args_addr)
            # write string
            utils.write_str(dram, a, args_addr + mem_begin - DRAM_OFF)
            args_addr += (len(a) + 1 + 7) & ~7
            if args_addr > ENV + 0x800:
                sys.exit("Not enough space for arguments")
        # null termination
        utils.write_u64(dram, argv + (mem_begin - DRAM_OFF) + argc * 8, 0)
        return args_addr

    def _tile_desc(self, tiles, i, vm):
        if i >= len(tiles):
            # mem size | TileAttr::IMEM | TileType::MEM
            return (DRAM_SIZE >> 12) << 28 | ((1 << 4) << 11) | 1

        tile_desc = 1 << 6  # RISCV
        if not vm:
            # mem size | TileAttr::IMEM
            tile_desc |= ((self.pmp_size >> 12) << 28) | ((1 << 4) << 11)
        if i < 5:
            tile_desc |= (1 << 1) << 11  # Rocket core
        else:
            tile_desc |= (1 << 0) << 11  # BOOM core
        if i == 6:
            tile_desc |= (1 << 2) << 11  # NIC
            tile_desc |= (1 << 3) << 11  # Serial
        return tile_desc
