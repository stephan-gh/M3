def build(gen, env):
    if env['TGT'] == 'host':
        env.m3_exe(gen, out = 'plasma-server', ins = ['server.cc'])
        env.m3_exe(gen, out = 'plasma-client', ins = ['client.cc'])
