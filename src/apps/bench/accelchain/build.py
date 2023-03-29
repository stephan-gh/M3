def build(gen, env):
    if env['TGT'] == 'gem5':
        env.m3_exe(gen, out='accelchain', ins=['accelchain.cc', 'direct.cc', 'indirect.cc'])
