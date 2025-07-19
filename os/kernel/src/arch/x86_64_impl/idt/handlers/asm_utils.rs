#![allow(unused)]

macro_rules! push_scratch {
    () => {
        r#"
            push r11
            push r10
            push r9
            push r8
            push rdi
            push rsi
            push rdx
            push rcx
            push rax
            "#
    };
}

macro_rules! pop_scratch {
    () => {
        r#"
            pop rax
            pop rcx
            pop rdx
            pop rsi
            pop rdi
            pop r8
            pop r9
            pop r10
            pop r11
            "#
    };
}

macro_rules! push_preserved {
    () => {
        r#"
            push r15
            push r14
            push r13
            push r12
            push rbp
            push rbx
            "#
    };
}

macro_rules! pop_preserved {
    () => {
        r#"
            pop rbx
            pop rbp
            pop r12
            pop r13
            pop r14
            pop r15
            "#
    };
}
pub(crate) use pop_preserved;
pub(crate) use pop_scratch;
pub(crate) use push_preserved;
pub(crate) use push_scratch;
