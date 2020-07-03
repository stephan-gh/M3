def build(gen, env):
    env = env.clone()
    # don't use LTO here because it makes the code >40% slower (WTF??)
    env.remove_flag('CXXFLAGS', '-flto')
    env.m3_exe(gen, out = 'wc', ins = ['loop.cc', 'wc.cc'])
