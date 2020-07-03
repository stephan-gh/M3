def build(gen, env):
    if env['TGT'] == 'gem5':
        lib = env.static_lib(gen, out = 'libpci', ins = env.glob('*.cc'))
        env.install(gen, env['LIBDIR'], lib)
