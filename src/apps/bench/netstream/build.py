def build(gen, env):
    env.m3_exe(gen, out = 'netstream-client', ins = ['client.cc'])
    env.m3_exe(gen, out = 'netstream-server', ins = ['server.cc'])
