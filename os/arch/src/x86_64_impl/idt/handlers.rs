use x86_64::structures::idt::{Entry, EntryOptions, HandlerFuncType};

pub struct PanicHandler {}

impl PanicHandler {
    pub fn set_handler<'a, F>(entry: &'a mut Entry<F>) -> &'a mut EntryOptions {
        unsafe {
            entry.set_handler_addr(x86_64::VirtAddr::from_ptr(
                PanicHandler::implementation as *const (),
            ))
        }
    }

    extern "C" fn implementation() -> ! {
        unimplemented!()
    }
}
