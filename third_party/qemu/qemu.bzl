def _qemu_runner_impl(ctx):
    # Construct the base command 
    cmd = cmd_args(ctx.attrs.qemu_binary[RunInfo])
    cmd.add("-L", ctx.attrs.pc_bios)
    bios = "uncompressed/edk2-{}-code.fd".format(ctx.attrs.arch)
    if ctx.attrs.arch == "riscv":
        cmd.add("-M", "virt")
    cmd.add("-cdrom", ctx.attrs.iso)
    if not ctx.attrs.legacy_bios:
        cmd.add("-drive", cmd_args(ctx.attrs.pc_bios, format="if=pflash,format=raw,readonly=on,file={}/" + bios))
    cmd.add(*ctx.attrs.args)
 
    return [
        DefaultInfo(),
        # RunInfo is what buck2 executes when you call `buck2 run`
        RunInfo(args = cmd),
    ]

qemu_runner = rule(
    impl = _qemu_runner_impl,
    attrs = {
        "qemu_binary": attrs.default_only(attrs.exec_dep(default = select({
            "prelude//cpu:x86_64": "//third_party/qemu:qemu-x86_64",
            "prelude//cpu:riscv64": "//third_party/qemu:qemu-riscv64",
        }))),
        "pc_bios": attrs.source(default = "//third_party/qemu:pc-bios"),
        "arch": attrs.default_only(attrs.string(default = select({
            "prelude//cpu:x86_64": "x86_64",
            "prelude//cpu:riscv64": "riscv",
        }))),
        "iso": attrs.source(),
        "legacy_bios": attrs.bool(default = False),
        "args": attrs.list(attrs.string(), default = []),
    },
)
