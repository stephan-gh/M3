def build(gen, env):
    env.m3_exe(gen, out='queuesrv', ins=['server.cc'])
    env.m3_exe(gen, out='queuecli', ins=['client.cc'])
