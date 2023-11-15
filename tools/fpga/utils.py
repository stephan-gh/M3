from noc import NoCethernet


def read_u64(mod, addr: int) -> int:
    return mod.mem[addr]


def write_u64(mod, addr: int, value: int):
    mod.mem[addr] = value


def read_str(mod, addr: int, length: int) -> str:
    b = mod.mem.read_bytes(addr, length)
    return b.decode()


def write_str(mod, string: str, addr: int):
    buf = bytearray(string.encode())
    buf += b'\x00'
    mod.mem.write_bytes(addr, bytes(buf), burst=False)  # TODO enable burst


def glob_addr(tile: int, offset: int) -> int:
    return (0x4000 + tile) << 49 | offset


def send_input(nocif: NoCethernet, chip: int, tile: int, ep: int, data: bytes):
    nocif.send_bytes((chip, tile), ep, data)
