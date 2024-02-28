dirs = [
    "apps",
    "kernel",
    "libs",
    "rot",
    "tilemux",
    "server",
    "fs", # generate the file systems last
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)

    # now that we know the rust crates to build, generate build edge to build the workspace with cargo
    env.m3_cargo_ws(gen)
