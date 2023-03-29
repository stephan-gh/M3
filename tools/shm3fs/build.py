def build(gen, env):
    bin = env.cxx_exe(gen, out='shm3fs', ins=['shm3fs.cc'])
    env.install(gen, env['TOOLDIR'], bin)
