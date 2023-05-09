{ nixpkgs ? <nixpkgs>, system ? builtins.currentSystem }:
with import nixpkgs { inherit system; };

let
	# building gem5
	gem5Inputs = [ scons gcc python3 zlib.dev ];

	# building the C cross compiler
	crossInputs = [ gcc python3 gmp.dev gmp mpfr.dev mpfr libmpc ncurses.dev ncurses texinfo ] ++
		lib.optional stdenv.isDarwin (runCommand "CoreFoundation" {} ''
			# I think the vanilla CoreFoundation package should add its frameworks search path
			# but it doesn’t, so we stitch together a new package here
			mkdir -p $out/nix-support
			echo NIX_LDFLAGS+=\" -F$out/Library/Frameworks\" > $out/nix-support/setup-hook
			ln -s ${darwin.apple_sdk.frameworks.CoreFoundation}/Library $out/Library
		'');

	# building the M3 system and applications
	m3Inputs = [ rustup ninja clang ];

	# build system support on Darwin
	darwinInputs = lib.attrValues {
		inherit gawk;
		nproc = writeScriptBin "nproc" ''#!/bin/sh
			exec sysctl -n hw.activecpu
		'';
	};

in mkShellNoCC {

	packages = gem5Inputs ++ crossInputs ++ m3Inputs ++
		lib.optionals stdenv.isDarwin darwinInputs;

	hardeningDisable = [ "format" ];  # breaks cross-gcc build
		
	shellHook = ''
		unset CC CXX AS LD AR RANLIB NM  # having these set breaks the cross-gcc build

		export RUSTUP_HOME=$PWD/.rustup
		export CARGO_HOME=$PWD/.cargo
		export M3_TARGET=''${M3_TARGET:-gem5}
		export M3_ISA=''${M3_ISA:-riscv}
		export M3_BUILD=''${M3_BUILD:-release}

		test -r ~/.shellrc && . ~/.shellrc
	'';
}
