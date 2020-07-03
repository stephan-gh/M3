def build(gen, env):
    env.m3_exe(gen, out = 'netbandwidth-client', ins = ['client.cc'])
    env.m3_exe(gen, out = 'netbandwidth-server', ins = ['server.cc'])
