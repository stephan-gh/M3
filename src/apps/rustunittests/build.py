def build(gen, env):
    if env['TGT'] != 'hw' or env['BUILD'] != 'debug':
        env.m3_rust_exe(gen, out = 'rustunittests')
    else:
        print("Warning: ignoring rustunittests")
