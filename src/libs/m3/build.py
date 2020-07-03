def build(gen, env):
    lib = env.static_lib(
        gen,
        out = 'libm3',
        ins = env.glob('*.cc') + env.glob('*/*.cc') + env.glob('arch/' + env['PLATF'] + '/*.cc')
    )
    env.install(gen, env['LIBDIR'], lib)
