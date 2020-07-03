def build(gen, env):
    if env['TGT'] == 'hw':
        bin = env.m3_rust_exe(gen, out = 'peidle')
        env.install_as(gen, env['BINDIR'] + '/pemux', bin)
