def build(gen, env):
    if env['TGT'] != 'hw' or env['BUILD'] != 'debug':
        env.m3_rust_exe(gen, out = 'net', libs = ['thread', 'base', 'm3'])
    else:
        print("Warning: ignoring net")
