def build(gen, env):
    # libkecacc-xkcp is only needed if the hardware accelerator is not available
    # The library could be added conditionally if needed but it should not make
    # any difference if it is listed but ends up being unused.
    env.m3_rust_exe(gen, out='hashmux', libs=['kecacc-xkcp'], features=['kecacc/' + env['TGT']])
