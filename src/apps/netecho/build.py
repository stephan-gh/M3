def build(gen, env):
    env.m3_exe(gen, out = 'netecho-client', ins = ['client.cc'])
    env.m3_exe(gen, out = 'netecho-server', ins = ['server.cc'])
