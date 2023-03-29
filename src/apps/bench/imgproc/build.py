def build(gen, env):
    if env['TGT'] == 'gem5':
        env.m3_exe(gen, out='imgproc', ins=['direct.cc', 'imgproc.cc', 'indirect.cc'])
