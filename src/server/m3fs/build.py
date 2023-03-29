def build(gen, env):
    env.m3_rust_exe(gen, out='m3fs', libs=['thread'], dir='sbin')
