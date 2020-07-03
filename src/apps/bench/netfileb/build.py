def build(gen, env):
    env.m3_exe(gen, out = 'netfileb-client', ins = ['client.cc'])
    env.m3_exe(gen, out = 'netfileb-server', ins = ['server.cc'])
