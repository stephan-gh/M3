def build(gen, env):
    files = env.glob(env['ISA'] + '/*.*')

    lib = env.static_lib(gen, out = 'libgem5', ins = files)
    env.install(gen, env['LIBDIR'], lib)

    sf_env = env.clone()
    sf_env.soft_float()
    lib = sf_env.static_lib(gen, out = 'libgem5sf', ins = files)
    sf_env.install(gen, sf_env['LIBDIR'], lib)
