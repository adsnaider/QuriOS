# Setup
We use buck2 for building all of the code, generating ISOs, emulation, testing,
etc. Follow their instructions for installation [here](https://buck2.build/docs/getting_started/).

# Platform Support
The build is meant to be as hermetic as possible, but we currently only support
Linux x86-64 and partially MacOS aarch64 (untested). Feel free to open an
issue if your build isn't working from the getgo.

## Adding a new Platform
To add a new platform, you will need to build some of the prebuilt artifacts
manually. I don't have direct instructions for this, but you can find the
prebuilt artifacts in <https://github.com/adsnaider/QuriOS-prebuilts> to get an
idea. Once that's built and released, you can update any instances of http_*
rules with `QuriOS-prebuilts` in the source to point to the new targets

# Building
As I mentioned, we use Buck2 for building, testing, and running code.

A few notworthy targets are:

* Run all host tests: `buck2 test //...`
* Build ISO image: `buck2 build //qurios:iso --target-platforms=//platforms:[riscv64|x86_64]` 
* Run the emulator: `buck2 run //qurios:emulator --target-platforms=//platforms:[riscv64|x86_64]`

> **Note:** You can omit the `--target-platforms` argument if you are targetting
> your host's architecture.

You can control build optimizations with `-m [release|debug]` added to the above
commands. The default build at the moment is `debug`.

# 3rd Party

## Tools
If you need a 3rd party tool added, you will need to add it to the
[QuriOS-prebuilts](https://github.com/adsnaider/QuriOS-prebuilts) repo for each
host x target supported.

## Rust Crates
Crates are added to the Cargo.toml and updated with `reindeer buckify`. You will
need to install [reindeer](https://github.com/facebookincubator/reindeer) to do
so.

# Updating rustc
If it's time to upgrade the Rust toolchain, you will need to do so in
`//toolchains/rust_toolchain/scripts`. Run `./fetch.py stable` to fetch the
stable toolchain to the latest version. Update //toolchains/BUCK to use the
new Rust version that was fetched.
