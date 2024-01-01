from ninjapie import BuildPath, SourcePath

outs = [
    "blau",
    "brom",
    "rosa",
]


def build(gen, env):
    if env['ISA'] != 'riscv':
        print('Root of Trust is only supported on RISC-V at the moment. Skipping.')
        # (because of riscv-rt and the RISC-V specific assembly code)
        return
    if env['TGT'] == 'hw22':
        print('Root of Trust cannot be built for hw22 at the moment. Skipping.')
        # (rosa uses the TCU TileDesc register, which is not available on hw22)
        return

    env = env.clone()
    env.soft_float()

    env['BINDIR'] = env['BUILDDIR'] + '/rotbin'
    env['CRGFLAGS'] += ['--features', 'rosa/' + env['TGT']]

    # FIXME: The RoT cannot be built in debug mode at the moment.
    # There are two issues:
    #   1. The RoT layers have fixed memory regions that are too small for
    #      the unoptimized debug builds.
    #   2. Unused code that makes use of heap allocations is not properly
    #      discarded in debug builds, so the RoT layers without heap run into
    #      linker errors (e.g. "undefined hidden symbol: __rdl_alloc").
    if env['BUILD'] == 'debug':
        print('Root of Trust cannot be built in debug mode at the moment. '
              'Building RoT layers in release mode instead.')
        env['BUILD'] = 'release'
        env['CRGFLAGS'] += ['--release']

    # riscv64imac-unknown-none-elf works too and is a standard Rust target
    env['TRIPLE'] = 'riscv64imc-unknown-none-elf'
    if env['TRIPLE'] == 'riscv64imc-unknown-none-elf':
        # Non-standard target, need to build the standard library ourselves
        env['CRGFLAGS'] += ['-Z build-std=core,alloc']
        # Can be used to completely remove panic messages from the binary
        # env['CRGFLAGS'] += ['-Z build-std-features=panic_immediate_abort']

    # clang wants the --target to be specified without the RISC-V extensions
    # (i.e. riscv64-... instead of riscv64imac-...). Currently, the cc Rust
    # crate (used by minicov) only handles this for riscv64gc, but not other
    # variants such as riscv64imac. Override the clang target explicitly to
    # avoid "unknown target triple 'riscv64imac-unknown-none-elf'".
    # https://github.com/rust-lang/cc-rs/blob/2447a2ba5f455c00b1563193e125b60eebbd8ebe/src/lib.rs#L1885-L1892
    if 'TARGET_CFLAGS' in env['CRGENV']:
        env['CRGENV']['TARGET_CFLAGS'] += ' --target=riscv64-unknown-none-elf'

    cargo_ws(env, gen, outs=outs)
    env.sub_build(gen, 'ubrom')


def cargo_ws(env, gen, outs):
    env = env.clone()

    deps = env.rust_deps()
    deps += [SourcePath.new(env, '.cargo/config'), SourcePath.new(env, 'Cargo.lock')]
    deps += env.glob(gen, '**/Cargo.toml')
    deps += env.glob(gen, '**/*.rs')
    deps += env.glob(gen, '**/*.ld')

    env['CRGFLAGS'] += ['--target', env['TRIPLE']]
    env.add_rust_features()

    outs = env.rust(gen, outs, deps)
    for o in outs:
        env.install(gen, outdir=env['BINDIR'], input=o)
        # Install as raw binary as well for the RoT layers
        bin = env.objcopy(gen, BuildPath.with_file_ext(env, o, 'bin'), o, type='binary')
        env.install(gen, env['BINDIR'], bin)
    return outs
