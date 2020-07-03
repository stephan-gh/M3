dirs = [
    'accelchain',
    'bench-apps',
    'compute',
    'cppbenchs',
    'fstrace',
    'imgproc',
    'loadgen',
    'netbandwidth',
    'netfile',
    'netfileb',
    'netlatency',
    'netstream',
    'rustbenchs',
    'scale',
    'scale-pipe',
    'tlbmiss',
    'vpe-clone',
    'vpe-exec',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
