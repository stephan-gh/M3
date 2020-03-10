dirs = [
    'arith',
    'console',
    'disk',
    'm3fs',
    'm3fsrs',
    'net',
    'pager',
    'pipes',
    'root',
    'timer',
    'vterm',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
