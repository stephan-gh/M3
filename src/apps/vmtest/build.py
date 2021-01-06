def build(gen, env):
    if env['TGT'] == 'hw':
        env.m3_rust_exe(gen, out = 'vmtest', libs = ['isr'], ldscript = 'pemux', varAddr = False)
