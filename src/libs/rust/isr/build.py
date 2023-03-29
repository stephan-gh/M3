def build(gen, env):
    files = ['src/' + env['ISA'] + '/Entry.S']

    lib = env.static_lib(gen, out='isr', ins=files)
    env.install(gen, env['LIBDIR'], lib)

    sf_env = env.clone()
    sf_env.soft_float()
    lib = sf_env.static_lib(gen, out='isrsf', ins=files)
    sf_env.install(gen, sf_env['LIBDIR'], lib)
