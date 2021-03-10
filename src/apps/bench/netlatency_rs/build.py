def build(gen, env):
    env.m3_exe(gen, out = 'netlatency-rs-client', ins = ['client.cc'])
    env.m3_exe(gen, out = 'netlatency-rs-server', ins = ['server.cc'])
