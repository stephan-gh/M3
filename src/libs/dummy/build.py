def build(gen, env):
    obj = env.cxx(gen, out = 'dummy.o', ins = ['dummy.cc'])
    for n in ['libm', 'libgloss']:
        lib = env.static_lib(gen, out = n, ins = [obj])
        env.install(gen, env['LIBDIR'], lib)
