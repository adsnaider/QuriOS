#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use derive_more::{Display, Error, TryFrom};

    use crate::caps::sync_ipc::{ExceptionAbi, SyncAbi};

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

    #[repr(usize)]
    #[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
    pub enum ExceptionInfo {
        DivideError,
        Debug,
        NonMaskableInterrupt,
        Breakpoint,
        Overflow,
        BoundRangeExceeded,
        InvalidOpCode,
        DeviceNotAvailable,
        InvalidTss { code: u64 },
        SegmentNotPresent { code: u64 },
        StackSegmentFault { code: u64 },
        GeneralProtection { code: u64 },
        X87FloatingPoint,
        AlignmentCheck { code: u64 },
        SimdFloatingPoint,
        Virtualization,
        VmmCommunicationException { code: u64 },
        SecurityException { code: u64 },
        CpProtectionException { code: u64 },
        HvInjectionException,
        PageFault { at: *mut (), code: u64 },
        DoubleFault,
        MachineCheck,
    }

    #[derive(Error, Display, Debug)]
    pub enum InvalidExceptionArgs {
        #[display("Invalid exception number {_0}")]
        InvalidException(#[error(not(source))] usize),
    }

    impl TryFrom<<ExceptionAbi as SyncAbi>::Args> for ExceptionInfo {
        type Error = InvalidExceptionArgs;

        fn try_from(value: <ExceptionAbi as SyncAbi>::Args) -> Result<Self, Self::Error> {
            let code = value.code;
            match value.kind {
                ExceptionKind::DIVIDE_ERROR => Ok(Self::DivideError),
                ExceptionKind::DEBUG => Ok(Self::Debug),
                ExceptionKind::NON_MASKABLE_INTERRUPT => Ok(Self::NonMaskableInterrupt),
                ExceptionKind::BREAKPOINT => Ok(Self::Breakpoint),
                ExceptionKind::OVERFLOW => Ok(Self::Overflow),
                ExceptionKind::BOUND_RANGE_EXCEEDED => Ok(Self::BoundRangeExceeded),
                ExceptionKind::INVALID_OP_CODE => Ok(Self::InvalidOpCode),
                ExceptionKind::DEVICE_NOT_AVAILABLE => Ok(Self::DeviceNotAvailable),
                ExceptionKind::INVALID_TSS => Ok(Self::InvalidTss { code }),
                ExceptionKind::SEGMENT_NOT_PRESENT => Ok(Self::SegmentNotPresent { code }),
                ExceptionKind::STACK_SEGMENT_FAULT => Ok(Self::StackSegmentFault { code }),
                ExceptionKind::GENERAL_PROTECTION => Ok(Self::GeneralProtection { code }),
                ExceptionKind::X87_FLOATING_POINT => Ok(Self::X87FloatingPoint),
                ExceptionKind::ALIGNMENT_CHECK => Ok(Self::AlignmentCheck { code }),
                ExceptionKind::SIMD_FLOATING_POINT => Ok(Self::SimdFloatingPoint),
                ExceptionKind::VIRTUALIZATION => Ok(Self::Virtualization),
                ExceptionKind::VMM_COMMUNICATION_EXCEPTION => {
                    Ok(Self::VmmCommunicationException { code })
                }
                ExceptionKind::SECURITY_EXCEPTION => Ok(Self::SecurityException { code }),
                ExceptionKind::CP_PROTECTION_EXCEPTION => Ok(Self::CpProtectionException { code }),
                ExceptionKind::HV_INJECTION_EXCEPTION => Ok(Self::HvInjectionException),
                ExceptionKind::PAGE_FAULT => Ok(Self::PageFault {
                    code,
                    at: value.extra as usize as *mut (),
                }),
                ExceptionKind::DOUBLE_FAULT => Ok(Self::DoubleFault),
                ExceptionKind::MACHINE_CHECK => Ok(Self::MachineCheck),
                other => Err(InvalidExceptionArgs::InvalidException(other)),
            }
        }
    }
}
