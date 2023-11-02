def build(gen, env):
    env = env.clone()
    env['CXXFLAGS'] += ['-fno-exceptions -fno-rtti']
    env['LINKFLAGS'] += ['-fno-exceptions -fno-rtti']

    files = ['Thread.cc', 'ThreadManager.cc']
    files += ['isa/' + env['ISA'] + '/ThreadSwitch.S']
    files += ['isa/' + env['ISA'] + '/Thread.cc']
    lib = env.static_lib(gen, out='thread', ins=files)
    env.install(gen, env['LIBDIR'], lib)
    env.install(gen, env['LXLIBDIR'], lib)
