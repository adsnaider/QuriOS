def _baremetal_transition_impl(ctx: AnalysisContext) -> list[Provider]:
    os_setting = ctx.attrs.os_setting
    none_os = ctx.attrs.none_os

    def _transition_impl_with_refs(platform: PlatformInfo) -> PlatformInfo:
        os_info = os_setting[ConstraintSettingInfo]
        none_info = none_os[ConstraintValueInfo]
        
        constraints = {
            setting: value
            for (setting, value) in platform.configuration.constraints.items()
            if setting != os_info.label
        }
        constraints[none_info.setting.label] = none_info

        return PlatformInfo(
            label = "transitioned-to-baremetal",
            configuration = ConfigurationInfo(
                constraints = constraints,
                values = platform.configuration.values,
            ),
        )

    return [
        DefaultInfo(),
        TransitionInfo(impl = _transition_impl_with_refs),
    ]

transition_to_baremetal = rule(
    impl = _baremetal_transition_impl,
    attrs = {
        "os_setting": attrs.dep(default = "prelude//os/constraints:os"),
        "none_os": attrs.dep(default = "prelude//os/constraints:none"),
    },
    is_configuration_rule = True,
)

# --- The QuriOS Userspace Equivalent ---

def _qurios_transition_impl(ctx: AnalysisContext) -> list[Provider]:
    os_setting = ctx.attrs.os_setting
    qurios_os = ctx.attrs.qurios_os

    def _tr(platform: PlatformInfo) -> PlatformInfo:
        os_info = os_setting[ConstraintSettingInfo]
        qurios_info = qurios_os[ConstraintValueInfo]
        
        constraints = {
            s: v for s, v in platform.configuration.constraints.items()
            if s != os_info.label
        }
        constraints[qurios_info.setting.label] = qurios_info

        return PlatformInfo(
            label = "transitioned-to-qurios",
            configuration = ConfigurationInfo(
                constraints = constraints,
                values = platform.configuration.values,
            ),
        )

    return [DefaultInfo(), TransitionInfo(impl = _tr)]

transition_to_qurios = rule(
    impl = _qurios_transition_impl,
    attrs = {
        "os_setting": attrs.dep(default = "prelude//os/constraints:os"),
        "qurios_os": attrs.dep(default = "prelude//os/constraints:none"),
    },
    is_configuration_rule = True,
)
