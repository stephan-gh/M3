def build(gen, env):
    for size in [1, 1024 * 2048, 1024 * 4096, 1024 * 8192]:
        myenv = env.clone()
        myenv['CXXFLAGS'] += ['-DDUMMY_BUF_SIZE=' + str(size)]
        obj = myenv.cxx(gen, out = 'vpe-clone-' + str(size) + '.o', ins = ['vpe-clone.cc'])
        myenv.m3_exe(gen, out = 'bench-vpe-clone-' + str(size), ins = [obj])
