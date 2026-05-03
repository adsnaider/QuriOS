def _generic_transition(ctx: AnalysisContext) -> list[Provider]:
    new_constraints = ctx.attrs.new_constraints

    def _transition_impl_with_refs(platform: PlatformInfo) -> PlatformInfo:
        
        constraints = {
            setting: value
            for (setting, value) in platform.configuration.constraints.items()
        }
        for (constraint, value) in new_constraints.items():
            constraint = constraint[ConstraintSettingInfo]
            value = value[ConstraintValueInfo]
            constraints[constraint.label] = value

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

generic_transition = rule(
    impl = _generic_transition,
    attrs = {
        "new_constraints": attrs.dict(key = attrs.dep(), value = attrs.dep()),
    },
    is_configuration_rule = True,
)

def transition_to_baremetal():
    generic_transition(
        new_constraints = {
            "prelude//os/constraints:os": "prelude//os/constraints:none"
        },
    )
def transition_to_qurios():
    generic_transition(
        new_constraints = {
            "prelude//os/constraints:os": "prelude//os/constraints:none"
        },
    )
