def build(gen, env):
    env = env.clone()
    env['CXXFLAGS']  += ['-fno-exceptions -fno-rtti']
    env['LINKFLAGS'] += ['-fno-exceptions -fno-rtti']
    lib = env.static_lib(
        gen,
        out = 'libthread',
        ins = ['Thread.cc', 'ThreadManager.cc'] + \
              ['isa/' + env['ISA'] + '/ThreadSwitch.S'] + \
              ['isa/' + env['ISA'] + '/Thread.cc']
    )
    env.install(gen, env['LIBDIR'], lib)
