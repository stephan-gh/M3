def build(gen, env):
    if env['TGT'] == 'host':
        env = env.clone()
        env['CXXFLAGS'] += ['-fno-exceptions']
        env.remove_flag('CXXFLAGS', '-flto')
        lib = env.static_lib(gen, out = 'libhost', ins = ['init.cc'])
        env.install(gen, env['LIBDIR'], lib)
