def build(gen, env):
    env.m3_exe(gen, out='bench-tlbmiss', ins=['bench-tlbmiss.cc'])
