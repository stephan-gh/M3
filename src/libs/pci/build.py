def build(gen, env):
    if env['TGT'] == 'gem5':
        lib = env.static_lib(gen, out='pci', ins=['Device.cc'])
        env.install(gen, env['LIBDIR'], lib)
