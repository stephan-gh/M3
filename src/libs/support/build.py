from glob import glob
import src.tools.ninjagen as ninjagen
import os

def is_our(ours, file):
    for o in ours:
        if os.path.basename(o) == os.path.basename(file):
            return True
    return False

def build(gen, env):
    if env['TGT'] != 'host':
        ours = []
        for f in env.glob(env['ISA'] + '/*.S'):
            obj = env.asm(gen, out = ninjagen.BuildPath.with_ending(env, f, '.o'), ins = [f])
            ours.append(env.install(gen, env['LIBDIR'], obj))

        for f in glob(env['SYSGCCLIBPATH'] + '/crt*'):
            if not is_our(ours, f):
                env.install(gen, env['LIBDIR'], ninjagen.SourcePath(f))
