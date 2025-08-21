#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use derive_more::{Display, TryFrom};

    #[repr(usize)]
    #[derive(Debug, Display, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash, TryFrom)]
    #[try_from(repr)]
    pub enum ExceptionKind {
        DivideError,
        Debug,
        NonMaskableInterrupt,
        Breakpoint,
        Overflow,
        BoundRangeExceeded,
        InvalidOpCode,
        DeviceNotAvailable,
        InvalidTss,
        SegmentNotPresent,
        StackSegmentFault,
        GeneralProtection,
        X87FloatingPoint,
        AlignmentCheck,
        SimdFloatingPoint,
        Virtualization,
        VmmCommunicationException,
        SecurityException,
        CpProtectionException,
        HvInjectionException,
        PageFault,
        DoubleFault,
        MachineCheck,
    }

    impl ExceptionKind {
        pub const DIVIDE_ERROR: usize = Self::DivideError as usize;
        pub const DEBUG: usize = Self::Debug as usize;
        pub const NON_MASKABLE_INTERRUPT: usize = Self::NonMaskableInterrupt as usize;
        pub const BREAKPOINT: usize = Self::Breakpoint as usize;
        pub const OVERFLOW: usize = Self::Overflow as usize;
        pub const BOUND_RANGE_EXCEEDED: usize = Self::BoundRangeExceeded as usize;
        pub const INVALID_OP_CODE: usize = Self::InvalidOpCode as usize;
        pub const DEVICE_NOT_AVAILABLE: usize = Self::DeviceNotAvailable as usize;
        pub const INVALID_TSS: usize = Self::InvalidTss as usize;
        pub const SEGMENT_NOT_PRESENT: usize = Self::SegmentNotPresent as usize;
        pub const STACK_SEGMENT_FAULT: usize = Self::StackSegmentFault as usize;
        pub const GENERAL_PROTECTION: usize = Self::GeneralProtection as usize;
        pub const X87_FLOATING_POINT: usize = Self::X87FloatingPoint as usize;
        pub const ALIGNMENT_CHECK: usize = Self::AlignmentCheck as usize;
        pub const SIMD_FLOATING_POINT: usize = Self::SimdFloatingPoint as usize;
        pub const VIRTUALIZATION: usize = Self::Virtualization as usize;
        pub const VMM_COMMUNICATION_EXCEPTION: usize = Self::VmmCommunicationException as usize;
        pub const SECURITY_EXCEPTION: usize = Self::SecurityException as usize;
        pub const CP_PROTECTION_EXCEPTION: usize = Self::CpProtectionException as usize;
        pub const HV_INJECTION_EXCEPTION: usize = Self::HvInjectionException as usize;
        pub const PAGE_FAULT: usize = Self::PageFault as usize;
        pub const DOUBLE_FAULT: usize = Self::DoubleFault as usize;
        pub const MACHINE_CHECK: usize = Self::MachineCheck as usize;
    }
}
