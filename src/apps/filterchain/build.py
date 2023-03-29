def build(gen, env):
    env.m3_exe(gen, out='filterchain', ins=['filterchain.cc'])
