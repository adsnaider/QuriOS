ENTRY(_start)

PHDRS
{
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

    /* Empty page to act as a guard */
    . = 0x1000;

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
