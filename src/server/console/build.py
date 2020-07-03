import os

def build(gen, env):
    # TODO the header file check is not platform independent
    if env['TGT'] == 'host' and os.path.isfile('/usr/include/X11/Xlib.h'):
        env.m3_exe(
            gen,
            out = 'console',
            ins = ['console.cc', 'Scancodes.cc', 'VGAConsole.cc'],
            libs = ['X11', 'rt']
        )
