fn main() {
    // Tell cargo to pass the linker script to the linker..
    println!("cargo:rustc-link-arg=-Tos/kernel/linker.ld");
}
