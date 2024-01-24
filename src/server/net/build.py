def build(gen, env):
    if env['TGT'] in ['hw', 'hw22', 'hw23']:
        libs = ['axieth', 'base', 'supc++']
    else:
        libs = []
    env.m3_rust_exe(gen, out='net', libs=libs, dir='sbin', features=['net/' + env['TGT']])
