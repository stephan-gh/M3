import src.tools.ninjagen as ninjagen

def build(gen, env):
    if env.try_execute('byacc -V'):
        gen.add_rule('byacc', ninjagen.Rule(
            cmd = 'byacc -B -d -o $out $in',
            desc = 'BYACC $out'
        ))
        gen.add_build(ninjagen.BuildEdge(
            'byacc',
            outs = [ninjagen.SourcePath.new(env, 'parser.tab.c')],
            ins = [ninjagen.SourcePath.new(env, 'cmds.y')],
        ))
    else:
        print('Cannot execute byacc, skipping regeneration of parser.tab.{c,h}...')

    parse_env = env.clone()
    parse_env['CFLAGS'] += ['-Wno-unused-parameter']
    parse_obj = parse_env.cc(gen, out = 'parser.tab.o', ins = ['parser.tab.c'])

    env.m3_exe(gen, out = 'shell', ins = env.glob('*.cc') + [parse_obj])
