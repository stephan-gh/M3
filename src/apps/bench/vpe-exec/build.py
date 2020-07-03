def build(gen, env):
    env.m3_exe(gen, out = 'bench-vpe-exec', ins = ['vpe-exec.cc'])
