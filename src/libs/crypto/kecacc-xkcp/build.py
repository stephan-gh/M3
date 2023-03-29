def build(gen, env):
    env = env.clone()
    env['CXXFLAGS'] += [
        '-std=c++17',           # C++17 is needed for template<auto ...>
        '-Wno-sign-conversion',  # silence warnings
    ]
    files = ['kecacc-xkcp.cc', 'xkcp/KeccakP-1600-opt64.c']
    lib = env.static_lib(gen, out='kecacc-xkcp', ins=files)
    env.install(gen, env['LIBDIR'], lib)
