def build(gen, env):
    env.m3_exe(gen, out='readelf', ins=['readelf.cc'])
