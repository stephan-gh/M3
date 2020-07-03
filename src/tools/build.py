dirs = [
    'elf2hex',
    'exm3fs',
    'gem52otf',
    'gem5log',
    'ignoreint',
    'm3fsck',
    'mkm3fs',
    'shm3fs',
]

def build(gen, env):
    for d in dirs:
        env.hostenv.sub_build(gen, d)
