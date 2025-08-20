#![cfg_attr(not(test), no_std)]

use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, Ordering};

#[repr(transparent)]
#[derive(Debug)]
pub struct StackNode<'a>(NonNull<StackNodeInner<'a>>);

#[repr(C, align(16))]
struct StackNodeInner<'a> {
    next: AtomicPtr<StackNodeInner<'a>>,
    size: usize,
    _buf: PhantomData<&'a ()>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StackNodeError {
    BufferTooSmall,
}

impl<'a> StackNode<'a> {
    pub const REQUIRED_ALIGNEMENT: usize = 16;
    pub const NODE_SIZE: usize = core::mem::size_of::<StackNodeInner<'static>>();

    pub fn new(buffer: &'a mut [u128]) -> Result<Self, StackNodeError> {
        const {
            assert!(core::mem::size_of::<StackNodeInner>() == core::mem::size_of::<u128>());
            assert!(core::mem::align_of::<StackNodeInner>() == core::mem::align_of::<u128>());
        }
        let inner = StackNodeInner {
            next: AtomicPtr::new(core::ptr::null_mut()),
            size: buffer.len(),
            _buf: PhantomData,
        };
        let bytes: u128 = unsafe { core::mem::transmute(inner) };
        let last = buffer.last_mut().ok_or(StackNodeError::BufferTooSmall)?;
        *last = bytes;
        Ok(Self(
            NonNull::new(last as *mut u128 as *mut StackNodeInner<'a>).unwrap(),
        ))
    }

    pub fn into_buffer(self) -> &'a mut [u128] {
        unsafe {
            let size = self.0.as_ref().size;
            core::slice::from_raw_parts_mut(self.0.as_ptr().sub(size).cast(), size + 1)
        }
    }

    unsafe fn from_inner(inner: NonNull<StackNodeInner<'a>>) -> Self {
        Self(inner)
    }

    fn inner(&self) -> NonNull<StackNodeInner<'a>> {
        self.0
    }
}

#[repr(transparent)]
pub struct StackList<'a> {
    head: AtomicPtr<StackNodeInner<'a>>,
}

impl Default for StackList<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StackList<'a> {
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    pub fn pop_front(&self) -> Option<StackNode<'a>> {
        match self
            .head
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |head| {
                if head.is_null() {
                    None
                } else {
                    unsafe { Some((*head).next.load(Ordering::SeqCst)) }
                }
            }) {
            Ok(stack) => Some(unsafe { StackNode::from_inner(NonNull::new_unchecked(stack)) }),
            Err(_) => None,
        }
    }

    pub fn push_front(&self, node: StackNode<'a>) {
        let node = node.inner();
        self.head
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |head| unsafe {
                node.as_ref().next.store(head, Ordering::SeqCst);
                Some(node.as_ptr())
            })
            .unwrap();
    }
}

#[macro_export]
macro_rules! stack_list_pop {
    () => {
        r#"
        mov rax, qword ptr [rdi]
    31:
        test rax, rax
        je 32f
        mov rcx, qword ptr [rax]
        lock cmpxchg qword ptr [rdi], rcx
        jne 31b
        jmp 33f
    32:
        xor     eax, eax
    33:
        "#
    };
}

#[macro_export]
macro_rules! stack_list_push {
    () => {
        r#"
        mov     rax, qword ptr [rdi]
    34:
        mov     rcx, rax
        xchg    qword ptr [rsi], rcx
        lock    cmpxchg qword ptr [rdi], rsi
        jne     34b
    "#
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let stacks = StackList::new();

        let mut buf1 = [0u128; 256];
        let buf1_ptr = &buf1 as *const _ as *const u8;
        let mut buf2 = [0u128; 256];
        let buf2_ptr = &buf2 as *const _ as *const u8;
        let mut buf3 = [0u128; 256];
        let buf3_ptr = &buf3 as *const _ as *const u8;
        let n1 = StackNode::new(buf1.as_u8_slice_mut()).unwrap();
        let n2 = StackNode::new(buf2.as_u8_slice_mut()).unwrap();
        let n3 = StackNode::new(buf3.as_u8_slice_mut()).unwrap();
        stacks.push_front(n1);
        stacks.push_front(n2);
        stacks.push_front(n3);

        let stack = stacks.pop_front().unwrap().into_buffer();
        assert_eq!(stack.len(), 256);
        assert_eq!(stack.as_ptr() as *const _ as *const u8, buf3_ptr);
        let stack = stacks.pop_front().unwrap().into_buffer();
        assert_eq!(stack.len(), 256);
        assert_eq!(stack.as_ptr() as *const _ as *const u8, buf2_ptr);
        let stack = stacks.pop_front().unwrap().into_buffer();
        assert_eq!(stack.len(), 256);
        assert_eq!(stack.as_ptr() as *const _ as *const u8, buf1_ptr);
    }

    #[test]
    fn multi_threaded() {
        let stacks = StackList::new();
        let mut buffers = [[0u128; 256]; 16];
        std::thread::scope(|scope| {
            for buf in &mut buffers {
                let stack = StackNode::new(buf.as_u8_slice_mut()).unwrap();
                stacks.push_front(stack);
            }
            for i in 0..16 {
                let stacks = &stacks;
                scope.spawn(move || {
                    let stack = stacks.pop_front().unwrap().into_buffer();
                    for _ in 0..8 {
                        for e in stack.iter_mut() {
                            *e = i;
                        }
                    }
                });
            }
        })
    }
}
