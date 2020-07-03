dirs = [
    "apps",
    "tools",
    "kernel",
    "libs",
    "pemux",
    "peidle",
    "server",
    "fs", # generate the file systems last
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
