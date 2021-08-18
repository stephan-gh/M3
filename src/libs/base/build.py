def build(gen, env):
    lib = env.static_lib(
        gen,
        out = 'libbase',
        ins = env.glob('*.cc') + env.glob('*/*.cc') + env.glob('arch/' + env['PLATF'] + '/*.cc')
    )
    env.install(gen, env['LIBDIR'], lib)
