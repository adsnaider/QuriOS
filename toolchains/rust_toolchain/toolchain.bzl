load("//manifests:targets.bzl", "TARGETS")
load("//manifests:renames.bzl", renames_entries = "ENTRIES")
load("//manifests:profiles.bzl", profiles_entries = "ENTRIES")
load("//manifests/stable:all.bzl", stable_index = "INDEX")
load("//manifests/beta:all.bzl", beta_index = "INDEX")
load("//manifests/nightly:all.bzl", nightly_index = "INDEX")
load("@prelude//rust:rust_toolchain.bzl", "PanicRuntime", "RustToolchainInfo")

DIST_ROOT = "https://static.rust-lang.org/dist"

_DEFAULT_TRIPLE = select({
    "prelude//os:linux": select({
        "prelude//cpu:arm64": "aarch64-unknown-linux-gnu",
        "prelude//cpu:riscv64": "riscv64gc-unknown-linux-gnu",
        "prelude//cpu:x86_64": "x86_64-unknown-linux-gnu",
    }),
    "prelude//os:macos": select({
        "prelude//cpu:arm64": "aarch64-apple-darwin",
        "prelude//cpu:x86_64": "x86_64-apple-darwin",
    }),
    "prelude//os:windows": select({
        "prelude//cpu:arm64": select({
            # Rustup's default ABI for the host on Windows is MSVC, not GNU.
            # When you do `rustup install stable` that's the one you get. It
            # makes you opt in to GNU by `rustup install stable-gnu`.
            "DEFAULT": "aarch64-pc-windows-msvc",
            "prelude//abi:gnu": "aarch64-pc-windows-gnu",
            "prelude//abi:msvc": "aarch64-pc-windows-msvc",
        }),
        "prelude//cpu:x86_64": select({
            "DEFAULT": "x86_64-pc-windows-msvc",
            "prelude//abi:gnu": "x86_64-pc-windows-gnu",
            "prelude//abi:msvc": "x86_64-pc-windows-msvc",
        }),
    }),
})

# Map from component name to the binary path inside the extracted tarball
_COMPONENT_ARTIFACTS = {
    "rustc": ["rustc/bin/rustc", "rustc/bin/rust-gdb", "rustc/bin/rust-gdbgui", "rustc/bin/rust-lldb", "rustc/bin/rustdoc"],
    "cargo": ["cargo/bin/cargo", "cargo/etc/bash_completion.d/cargo", "cargo/share/doc/cargo"],
    "clippy-preview": ["clippy-preview/bin/clippy-driver", "clippy-preview/share/doc/clippy"],
    "rustfmt-preview": ["rustfmt-preview/bin/rustfmt"],
    "miri-preview": ["miri-preview/bin/miri"],
    "rust-analyzer-preview": ["rust-analyzer-preview/bin/rust-analyzer"],
    "llvm-tools-preview": ["llvm-tools-preview/bin/llvm-objdump"],
}

# Friendly names for subtargets
_SHORT_NAMES = {
    "clippy-driver": "clippy",
    "rustfmt-preview": "rustfmt",
    "miri-preview": "miri",
    "rust-analyzer-preview": "rust-analyzer",
    "llvm-tools-preview": "llvm-tools",
    "llvm-bitcode-linker-preview": "llvm-bitcode-linker",
    "rustc-codegen-cranelift-preview": "rustc-codegen-cranelift",
    "rust-docs-json-preview": "rust-docs-json",
}

def _rust_toolchain_impl(
    ctx: AnalysisContext,
) -> list[Provider]:
    manifest = _uncompress_manifest(ctx.attrs._channel, ctx.attrs._manifest)

    # Look up which components this profile includes
    profile_components = manifest.profiles.get(ctx.attrs._profile, None)
    if profile_components == None:
        fail("Unknown profile '{}'. Available: {}".format(ctx.attrs._profile, manifest.profiles.keys()))


    resolved_components = []
    for comp in profile_components:
        actual = manifest.renames.get(comp, comp)
        resolved_components.append(actual)


    sub_targets = {}
    sysroot_srcs = {}
    for component_name in resolved_components:
        pkg = manifest.pkgs.get(component_name, None)
        if pkg == None:
            continue

        is_target_component = component_name in ["rust-std", "rust-docs-json-preview"]
        triple = ctx.attrs.rustc_target_triple if is_target_component else ctx.attrs.rustc_exec_triple

        # Try the specified target first, then wildcard "*"
        target_info = pkg.get(triple, pkg.get("*", None))
        if target_info == None:
            continue

        component = _download_rust_component(
            ctx,
            component_name,
            target_info.url,
            target_info.sha256
        )

        short_name = _SHORT_NAMES.get(component_name, component_name)
        sub_targets[short_name] = [DefaultInfo(default_output = component)]

        artifacts = _COMPONENT_ARTIFACTS.get(component_name, [])

        for artifact_path in artifacts:
            artifact = component.project(artifact_path)

            parts = artifact_path.split("/")
            sub_targets[parts.pop()] = [RunInfo([artifact]), DefaultInfo(default_output = artifact)]

        if component_name == "rust-std":
            sysroot_srcs["lib"] = component.project("rust-std-{}/lib".format(ctx.attrs.rustc_target_triple))

    sysroot_ident = "{}-{}-{}-sysroot".format(ctx.attrs._channel, ctx.attrs.rustc_target_triple, ctx.attrs._profile)
    sysroot = ctx.actions.symlinked_dir(sysroot_ident, sysroot_srcs)

    return [
        DefaultInfo(
            default_output = sysroot,
            sub_targets = sub_targets,
        ),
        RustToolchainInfo(
            report_unused_deps = ctx.attrs.report_unused_deps,
            rustc_target_triple = ctx.attrs.rustc_target_triple,
            rustc_flags = ctx.attrs.rustc_flags,
            rustc_binary_flags = ctx.attrs.rustc_binary_flags,
            rustc_test_flags = ctx.attrs.rustc_test_flags,
            rustdoc_flags = ctx.attrs.rustdoc_flags,
            doctests = ctx.attrs.doctests,
            default_edition = ctx.attrs.default_edition,

            sysroot_path = sysroot,
            compiler = sub_targets["rustc"][0],
            rustdoc = sub_targets["rustdoc"][0],
            clippy_driver = sub_targets["clippy"][0],
            # miri_driver = sub_targets["miri"][0] if hasattr(sub_targets, "miri") else None,

            # "miri_sysroot_path": provider_field(Artifact | None, default = None),
            # "miri_flags": provider_field(list[typing.Any], default = []),

            allow_lints = ctx.attrs.allow_lints,
            warn_lints = ctx.attrs.warn_lints,
            deny_lints = ctx.attrs.deny_lints,
            deny_on_check_lints = ctx.attrs.deny_on_check_lints,

            panic_runtime = PanicRuntime(ctx.attrs.panic_runtime),
            nightly_features = ctx.attrs.nightly_features,

            # "rustc_env": provider_field(dict[str, typing.Any], default = {}),
            # "extra_rustc_flags": provider_field(list[typing.Any], default = []),
            # "rust_target_path": provider_field(Dependency | None, default = None),
            # "rustc_check_flags": provider_field(list[typing.Any], default = []),
            # "rustc_coverage_flags": provider_field(typing.Any, default = ("-Cinstrument-coverage",)),
            # "rustdoc_env": provider_field(dict[str, typing.Any], default = {}),
            # "llvm_lines_tool": provider_field(RunInfo | None, default = None),
            # "measureme_crox": provider_field(RunInfo | None, default = None),
            # "make_trace_upload": provider_field(typing.Callable[[Artifact], RunInfo] | None, default = None),
            # "configuration_hash": provider_field(str | None, default = None),
            # "rust_error_handler": provider_field(typing.Any, default = None),
            # "remarks": provider_field(str | None, default = None),
        ),
    ]

_rust_toolchain = rule(
    impl = _rust_toolchain_impl,
    attrs = {
        "clippy_toml": attrs.option(attrs.dep(providers = [DefaultInfo]), default = None),
        "default_edition": attrs.option(attrs.string(), default = None),
        "doctests": attrs.bool(default = False),
        "nightly_features": attrs.bool(default = False),
        "report_unused_deps": attrs.bool(default = False),
        "rustc_binary_flags": attrs.list(attrs.arg(), default = []),
        "rustc_flags": attrs.list(attrs.arg(), default = []),
        "rustc_exec_triple": attrs.string(default = _DEFAULT_TRIPLE),
        "rustc_target_triple": attrs.string(default = _DEFAULT_TRIPLE),
        "rustc_test_flags": attrs.list(attrs.arg(), default = []),
        "rustdoc_flags": attrs.list(attrs.arg(), default = []),
        "panic_runtime": attrs.string(default = "unwind"),

        "allow_lints": attrs.list(attrs.string(), default = []),
        "warn_lints": attrs.list(attrs.string(), default = []),
        "deny_lints": attrs.list(attrs.string(), default = []),
        "deny_on_check_lints": attrs.list(attrs.string(), default = []),

        "_channel": attrs.string(),
        "_profile": attrs.string(),
        "_manifest": attrs.any(),
    },
    is_toolchain_rule = True,
)

def _make_toolchain_rule(channel: str, profile: str, manifest: typing.Any):
    def fn(**kwargs):
        _rust_toolchain(
            _channel = channel,
            _profile = profile,
            _manifest = manifest,
            **kwargs
        )
    return fn

def _make_version_entry(channel, manifest):
    return struct(
        complete = _make_toolchain_rule(channel, "complete", manifest),
        default = _make_toolchain_rule(channel, "default",  manifest),
        minimal = _make_toolchain_rule(channel, "minimal",  manifest),
    )

def _make_channel(channel, index):
    def _entry_for_version(version, index):
        manifest = getattr(index, version, None)
        if manifest == None:
            fail("Unknown version '{}'. Available: {}".format(version, dir(index)))
        return _make_version_entry(channel, manifest)

    return struct(**{
        "version": lambda version: _entry_for_version(version, index),
        "latest": _make_version_entry(channel, index.latest)
    })

rust_toolchain = struct(
    stable = _make_channel("stable", stable_index),
    beta = _make_channel("beta", beta_index),
    nightly = _make_channel("nightly", nightly_index),
)

def _download_rust_component(
    ctx: AnalysisContext,
    component_name: str,
    url: str,
    sha256: str,
) -> Artifact:
    archive = ctx.actions.declare_output(f"{component_name}.tar.xz")
    ctx.actions.download_file(archive.as_output(), url, sha256 = sha256)

    # Extract — Rust tarballs contain a top-level directory
    output = ctx.actions.declare_output(component_name, dir = True)
    script, _ = ctx.actions.write(
        f"unpack_{component_name}.sh",
        [
            cmd_args(output, format = "mkdir -p {}"),
            cmd_args(output, format = "cd {}"),
            cmd_args("tar", "-xJf", archive, "--strip-components=1", delimiter = " ", relative_to = output),
        ],
        is_executable = True,
        allow_args = True,
    )
    ctx.actions.run(
        cmd_args(["/bin/sh", script], hidden = [archive, output.as_output()]),
        category = "rust_component",
        identifier = component_name,
        local_only = True,
    )
    return output


def _uncompress_manifest(channel, manifest):
    version = manifest["v"]
    date = manifest["d"]
    renames = renames_entries[manifest["r"]]
    profiles = profiles_entries[manifest["p"]] if "p" in manifest else {}

    default_url_version = version if channel == "stable" else channel

    # Reserved keys that are not package names
    reserved = ["v", "d", "r", "p"]

    pkgs = {}
    for pkg_name, pkg_data in manifest.items():
        if pkg_name in reserved:
            continue

        # The "u" key overrides the URL version for this package
        url_version = pkg_data.get("u", default_url_version)
        pkg_name_stripped = pkg_name.removesuffix("-preview")

        targets_info = {}
        for target_key, hash_or_ref in pkg_data.items():
            if target_key == "u":
                continue

            # Resolve target triple
            if target_key == "_":
                target_triple = "*"
                target_tail = ""
            else:
                target_triple = TARGETS[target_key]
                target_tail = "-" + target_triple

            # An integer value means forward to another target's URL
            if type(hash_or_ref) == "int":
                ref_key = "_{}".format(hash_or_ref)
                ref_triple = TARGETS[ref_key]
                if ref_triple in targets_info:
                    targets_info[target_triple] = targets_info[ref_triple]
                continue

            url = "{}/{}/{}-{}{}".format(DIST_ROOT, date, pkg_name_stripped, url_version, target_tail) + ".tar.xz"
            sha256_hex = hash_or_ref

            targets_info[target_triple] = struct(
                url = url,
                sha256 = sha256_hex,
            )

        pkgs[pkg_name] = targets_info

    return struct(
        version = version,
        date = date,
        renames = renames,
        profiles = profiles,
        pkgs = pkgs,
    )
