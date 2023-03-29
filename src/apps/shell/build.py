def build(gen, env):
    env.m3_exe(gen, out='shell', ins=env.glob(gen, '*.cc'))
