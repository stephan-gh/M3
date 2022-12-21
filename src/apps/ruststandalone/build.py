def build(gen, env):
    for d in ['stdasender', 'stdareceiver', 'vmtest']:
        env.sub_build(gen, d)
