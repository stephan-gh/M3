dirs = [
    'elf2hex',
    'exm3fs',
    'gem52otf',
    'gem5log',
    'hwitrace',
    'ignoreint',
    'm3fsck',
    'mkm3fs',
    'netdbg',
    'setpgrp',
    'shm3fs',
]


def build(gen, env):
    for d in dirs:
        env.hostenv.sub_build(gen, d)
