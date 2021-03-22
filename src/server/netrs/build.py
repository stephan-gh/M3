import os

def build(gen, env):
    if env['TGT'] != 'hw' or os.environ.get('M3_BUILD') != 'debug':
        env.m3_rust_exe(gen, out = 'netrs', libs = ['thread', 'base', 'm3'])
    else:
        print("Warning: ignoring netrs")
