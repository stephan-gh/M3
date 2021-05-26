def build(gen, env):
    env = env.clone()
    env.remove_flag('CXXFLAGS', '-flto')
    lib = env.static_lib(gen, out = 'libmemory', ins = env.glob('*.cc'))
    env.install(gen, env['LIBDIR'], lib)
