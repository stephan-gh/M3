import fcntl
import os
import sys
import threading

import select
import serial
from serial.tools.miniterm import Miniterm
import termios

from noc import NoCethernet
import memory

import loader
import utils


class Term:
    def __init__(self):
        pass


class TCUTerm(Term):
    # inspired by MiniTerm (https://github.com/pyserial/pyserial/blob/master/serial/tools/miniterm.py)
    def __init__(self, dram: memory, nocif: NoCethernet):
        self.fd = sys.stdin.fileno()
        # make stdin nonblocking
        fl = fcntl.fcntl(self.fd, fcntl.F_GETFL)
        fcntl.fcntl(self.fd, fcntl.F_SETFL, fl | os.O_NONBLOCK)
        # get original terminal attributes to restore them later
        self.old = termios.tcgetattr(self.fd)
        self.nocif = nocif
        self.dram = dram
        # reset tile and EP in case they are set from a previous run
        utils.write_u64(self.dram, loader.SERIAL_ADDR + 0, 0)
        utils.write_u64(self.dram, loader.SERIAL_ADDR + 8, 0)
        self._setup()

    def should_stop(self) -> bool:
        if sys.stdin in select.select([sys.stdin], [], [], 0)[0]:
            bytes = self._getkey()
            if len(bytes) == 1 and bytes[0] == chr(0x1d):
                return True
            self._write(bytes)
        return False

    def cleanup(self):
        termios.tcsetattr(self.fd, termios.TCSAFLUSH, self.old)

    def _setup(self):
        new = termios.tcgetattr(self.fd)
        new[3] = new[3] & ~(termios.ICANON | termios.ISIG | termios.ECHO)
        new[6][termios.VMIN] = 1
        new[6][termios.VTIME] = 0
        termios.tcsetattr(self.fd, termios.TCSANOW, new)
        print("-- TCU Terminal ( Quit: Ctrl+] ) --")

    def _getkey(self) -> bytes:
        try:
            # read multiple bytes to get sequences like ^[D
            bytes = sys.stdin.read(8)
        except KeyboardInterrupt:
            bytes = ['\x03']
        return bytes

    def _write(self, data: bytes):
        bytes = data.encode('utf-8')
        # read desired destination
        tile = utils.read_u64(self.dram, loader.SERIAL_ADDR + 0)
        ep = utils.read_u64(self.dram, loader.SERIAL_ADDR + 8)
        # only send if it was initialized
        if ep != 0:
            utils.send_input(self.nocif, tile >> 8, tile & 0xFF, ep, bytes)


class LxTerm(Term):
    def __init__(self, port: int):
        # interactive usage
        ser = serial.Serial(port=port, baudrate=115200, xonxoff=True)
        self.miniterm = Miniterm(ser)
        self.miniterm.raw = True
        self.miniterm.set_rx_encoding('UTF-8')
        self.miniterm.set_tx_encoding('UTF-8')

        def key_description(character) -> str:
            """generate a readable description for a key"""
            ascii_code = ord(character)
            if ascii_code < 32:
                return 'Ctrl+{:c}'.format(ord('@') + ascii_code)
            else:
                return repr(character)

        sys.stderr.write('--- Miniterm on {p.name}  {p.baudrate},{p.bytesize},{p.parity},{p.stopbits} ---\n'.format(
            p=self.miniterm.serial))
        sys.stderr.write('--- Quit: {} ---\n'.format(key_description(self.miniterm.exit_character)))

        # only start the miniterm writer (we'll never read, because all prints are done via TCU)
        self.miniterm.alive = True
        self.miniterm.transmitter_thread = threading.Thread(target=self.miniterm.writer, name='tx')
        self.miniterm.transmitter_thread.daemon = True
        self.miniterm.transmitter_thread.start()
        self.miniterm.console.setup()

    def should_stop(self) -> bool:
        return not self.miniterm.alive

    def cleanup(self):
        self.miniterm.stop()
        sys.stderr.write('\n--- exit ---\n')
        self.miniterm.close()
