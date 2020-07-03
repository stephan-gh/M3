def build(gen, env):
    env = env.clone()
    env['CXXFLAGS'] += ['-fno-exceptions']
    env.remove_flag('CXXFLAGS', '-flto')
    lib = env.static_lib(gen, out = 'libheap', ins = ['heap.cc'])
    env.install(gen, env['LIBDIR'], lib)
