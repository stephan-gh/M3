def build(gen, env):
    lib = env.static_lib(
        gen,
        out='m3',
        ins=env.glob(gen, '*.cc') + env.glob(gen, '*/*.cc')
    )
    env.install(gen, env['LIBDIR'], lib)
