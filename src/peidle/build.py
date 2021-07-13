def build(gen, env):
    if env['TGT'] == 'hw':
        env.m3_rust_exe(gen, out = 'peidle', ldscript = 'pemux', varAddr = False, libs = ['isr'])
