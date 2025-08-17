use proc_macro::TokenStream;
use syn::{Data, DeriveInput, parse_macro_input};

#[proc_macro_derive(SyscallRequest)]
pub fn syscall_request(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let Data::Struct(strt) = input.data else {
        panic!("Invalid SyscallRequest target: Must be a regular struct");
    };
    let fields = strt
        .fields
        .iter()
        .map(|f| f.ident.clone().expect("Missing struct's field name"));
    let fields2 = fields.clone();

    let num_fields = fields.len();

    let derivation = quote::quote! {
        impl crate::syscall::SyscallRequest for #name {
            fn into_args(self) -> crate::syscall::UninitSyscallParams {
                use ::core::mem::MaybeUninit;
                use crate::syscall::SYSCALL_ARGS;
                const {
                    if (#num_fields > SYSCALL_ARGS) {
                        panic!("Too many fields to generate a syscall request");
                    }
                }
                let mut args = [MaybeUninit::uninit(); SYSCALL_ARGS];
                let mut next_id = 0;
                #(
                   args[next_id] = MaybeUninit::new(Into::into(self.#fields));
                   next_id += 1;
                )*;
                args
            }

            fn try_from_args(args: &crate::syscall::InitSyscallParams) -> Result<Self, crate::caps::CapError> {
                const {
                    use crate::syscall::SYSCALL_ARGS;
                    if (#num_fields > SYSCALL_ARGS) {
                        panic!("Too many fields to generate a syscall request");
                    }
                }
                let mut next_id = 0;
                Ok(Self {
                    #(
                        #fields2: {
                            let value = args[next_id].try_into()?;
                            next_id += 1;
                            value
                        },
                    )*
                })
            }
        }
    };
    TokenStream::from(derivation)
}
