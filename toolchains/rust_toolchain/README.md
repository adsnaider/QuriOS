> moved to https://github.com/JonasKruckenberg/buck2tf


# hermetic buck2 rust toolchain

## Installation

Add this repo as an external cell in your projects .buckconfig

```ini
[cells]
  rust = rust

[external_cells]
  rust = git

[external_cell_rust]
  git_origin = https://github.com/JonasKruckenberg/buck2_rust_toolchain.git
  commit_hash = ...
```

## Configuring the toolchain

Import `rust_toolchain` and use it to set up a toolchain:

```starlark
load("@rust//:toolchain.bzl", "rust_toolchain")

# Latest stable rust, default profile.
rust_toolchain.stable.latest.default(
    name = "rust",
    visibility = ["PUBLIC"],
)
```

## Cheat sheet

- Latest stable or beta rust profile.

  ```starlark
  load("@rust//:toolchain.bzl", "rust_toolchain")
  
  # Latest stable rust, default profile.
  rust_toolchain.stable.latest.default(...)
  
  # Latest stable rust, minimal toolchain that has nothing but rustc, cargo, rust-std.
  rust_toolchain.stable.latest.minimal(...)
  
  # Latest beta toolchain, test upcoming compiler versions!
  rust_toolchain.beta.latest.default(...)
  rust_toolchain.beta.latest.minimal(...)
  ```

- Latest nightly rust profile.

  ```starlark
  # DO NOT ACTUALLY USE THIS, ALWAYS USE PINNED NIGHTLIES
  # but I'm not your dad, so here you go
  rust_toolchain.nightly.latest.default(...)
  ```

- A specific version of rust:

  ```starlark
  rust_toolchain.stable.version("1.48.0").default(...)
  rust_toolchain.beta.version("2021-01-01").default(...)
  rust_toolchain.nightly.version("2020-12-31").default(...)
  ```

## Cross-compiling

Just like the prelude `system_rust_toolchain` you can specify `rustc_target_triple` to 
get a cross-compiling toolchain for the specified target

```starlark
rust_toolchain.stable.version("1.48.0").default(
    name = "rust",
    rustc_target_triple = "riscv64gc-unknown-none-elf",
    visibility = ["PUBLIC"],
)
```
