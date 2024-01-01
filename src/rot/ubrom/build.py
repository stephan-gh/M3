from ninjapie import SourcePath


def build(gen, env):
    env = env.clone()
    ldscript = SourcePath.new(env, 'ubrom.ld')
    env['ASFLAGS'] += ['-no-pie']
    env['LINKFLAGS'] += ['-nodefaultlibs', '-nostartfiles', '-T', ldscript]
    bin = env.c_exe(gen, out='ubrom', ins=['ubrom.S'], deps=[ldscript])
    env.install(gen, env['BINDIR'], bin)
