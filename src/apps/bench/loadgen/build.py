def build(gen, env):
    env.m3_exe(gen, out='bench-loadgen', ins=['loadgen.cc'])
