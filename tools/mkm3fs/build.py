def build(gen, env):
    bin = env.cxx_exe(gen, out='mkm3fs', ins=['mkm3fs.cc'])
    env.install(gen, env['TOOLDIR'], bin)
