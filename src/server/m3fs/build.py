def build(gen, env):
    env.m3_exe(gen, out = 'm3fs', ins = env.glob('*.cc') + env.glob('*/*.cc'))
