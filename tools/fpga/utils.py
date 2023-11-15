def read_u64(mod, addr):
    return mod.mem[addr]


def write_u64(mod, addr, value):
    mod.mem[addr] = value


def read_str(mod, addr, length):
    b = mod.mem.read_bytes(addr, length)
    return b.decode()


def write_str(mod, str, addr):
    buf = bytearray(str.encode())
    buf += b'\x00'
    mod.mem.write_bytes(addr, bytes(buf), burst=False)  # TODO enable burst


def glob_addr(tile, offset):
    return (0x4000 + tile) << 49 | offset


def send_input(fpga_inst, chip, tile, ep, bytes):
    fpga_inst.nocif.send_bytes((chip, tile), ep, bytes)
