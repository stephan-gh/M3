def build(gen, env):
    env.m3_exe(gen, out = 'netbandwidth-rs-client', ins = ['client.cc'])
    env.m3_exe(gen, out = 'netbandwidth-rs-server', ins = ['server.cc'])
