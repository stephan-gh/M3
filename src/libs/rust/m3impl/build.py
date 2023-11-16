def build(gen, env):
    env.m3_rust_lib(gen, features=["m3impl/" + env['TGT']])
