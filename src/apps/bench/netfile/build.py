def build(gen, env):
    env.m3_exe(gen, out = 'netfile-client', ins = ['client.cc'])
    env.m3_exe(gen, out = 'netfile-server', ins = ['server.cc'])
