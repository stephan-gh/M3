def build(gen, env):
    env.m3_rust_exe(gen, out = 'vmtest', libs = ['isr'], ldscript = 'pemux', varAddr = False)
