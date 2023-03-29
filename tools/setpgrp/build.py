def build(gen, env):
    bin = env.cxx_exe(gen, out='setpgrp', ins=['setpgrp.cc'])
    env.install(gen, env['TOOLDIR'], bin)
