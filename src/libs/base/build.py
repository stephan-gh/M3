def build(gen, env):
    env = env.clone()
    env['CXXFLAGS'] += ['-fno-exceptions']

    lib = env.static_lib(
        gen,
        out = 'libbase',
        ins = env.glob('*.cc') + env.glob('*/*.cc')
    )
    env.install(gen, env['LIBDIR'], lib)
