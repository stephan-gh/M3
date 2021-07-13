def build(gen, env):
    env.m3_rust_exe(gen, out = 'stdareceiver', libs = ['isr'], ldscript = 'pemux', varAddr = False)
