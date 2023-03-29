def build(gen, env):
    bin = env.cxx_exe(gen, out='exm3fs', ins=['exm3fs.cc'])
    env.install(gen, env['TOOLDIR'], bin)
