ENTRY(kmain)

/* Define the program headers we want so the bootloader gives us the right */
/* MMU permissions */
PHDRS
{
    /* interrupt_stack PT_LOAD FLAGS((1 << 1) | (1<< 2)) /* Write + Read */
    text    PT_LOAD    FLAGS((1 << 0) | (1 << 2)) ; /* Execute + Read */
    rodata  PT_LOAD    FLAGS((1 << 2)) ;            /* Read only */
    data    PT_LOAD    FLAGS((1 << 1) | (1 << 2)) ; /* Write + Read */
    dynamic PT_DYNAMIC FLAGS((1 << 1) | (1 << 2)) ; /* Dynamic PHDR for relocations */
}

SECTIONS
{
    /* We wanna be placed in the topmost 2GiB of the address space, for optimisations */
    /* and because that is what the Limine spec mandates. */
    /* Any address in this region will do, but often 0xffffffff80000000 is chosen as */
    /* that is the beginning of the region. */
    . = 0xffffffff80000000;
    /* Empty page to act as a guard */
    . += CONSTANT(MAXPAGESIZE);

    /* .interrupt_stack : { */
        /* *(.interrupt_stack) */
    /* } :interrupt_stack */

    .text : ALIGN(CONSTANT(MAXPAGESIZE)){
        *(.text .text.*)
    } :text


    .rodata : ALIGN(CONSTANT(MAXPAGESIZE)) {
        *(.rodata .rodata.*)
    } :rodata

    .data : ALIGN(CONSTANT(MAXPAGESIZE)) {
        *(.data .data.*)
    } :data

    /* Dynamic section for relocations, both in its own PHDR and inside data PHDR */
    .dynamic : ALIGN(CONSTANT(MAXPAGESIZE)) {
        *(.dynamic)
    } :data :dynamic


    /* NOTE: .bss needs to be the last thing mapped to :data, otherwise lots of */
    /* unnecessary zeros will be written to the binary. */
    /* If you need, for example, .init_array and .fini_array, those should be placed */
    /* above this. */
    .bss : ALIGN(CONSTANT(MAXPAGESIZE)) {
        *(.bss .bss.*)
        *(COMMON)
    } :data


    /* Discard .note.* and .eh_frame since they may cause issues on some hosts. */
    /DISCARD/ : {
        *(.eh_frame)
        *(.note .note.*)
    }
}
