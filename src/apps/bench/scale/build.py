def build(gen, env):
    env.m3_exe(gen, out='bench-scale', ins=['scale.cc'])
