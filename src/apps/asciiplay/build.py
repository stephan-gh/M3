def build(gen, env):
    if env['TGT'] == 'host':
        env.m3_exe(gen, out = 'asciiplay', ins = ['asciiplay.cc'])
