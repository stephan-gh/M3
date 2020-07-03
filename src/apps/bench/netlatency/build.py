def build(gen, env):
    env.m3_exe(gen, out = 'netlatency-client', ins = ['client.cc'])
    env.m3_exe(gen, out = 'netlatency-server', ins = ['server.cc'])
