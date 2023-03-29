def build(gen, env):
    env = env.clone()
    env['CXXFLAGS'] += ['-fno-exceptions']

    lib = env.static_lib(
        gen,
        out='base',
        ins=env.glob(gen, '*.cc') + env.glob(gen, '*/*.cc')
    )
    env.install(gen, env['LIBDIR'], lib)
