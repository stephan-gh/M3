def build(gen, env):
    if env['PLATF'] == 'kachel':
        env.m3_rust_exe(gen, out = 'ruststandalone', ldscript = 'isr')
