def build(gen, env):
    if env['TGT'] == 'gem5':
        env.m3_rust_exe(gen, out='disk', dir='sbin')
