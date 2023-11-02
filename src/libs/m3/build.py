def build(gen, env):
    files = env.glob(gen, '*.cc') + env.glob(gen, '*/*.cc')
    lib = env.static_lib(gen, out='m3', ins=files + env.glob(gen, 'arch/m3/*.cc'))
    env.install(gen, env['LIBDIR'], lib)

    lx_env = env.clone()
    lx_env['CPPFLAGS'] += ['-D__m3lx__']
    lib = lx_env.static_lib(gen, out='m3-lx', ins=files + env.glob(gen, 'arch/linux/*.cc'))
    lx_env.install(gen, lx_env['LXLIBDIR'], lib)
