ENTRY(entry)
OUTPUT_FORMAT(elf64-x86-64) 

/* We want to be placed in the higher half, 2MiB above 0x00 in physical memory */
KERNEL_OFFSET = 0xFFFFFFFF80200000;

SECTIONS {
    . = KERNEL_OFFSET;

    __kernel_start = .;

    .limine_reqs : ALIGN(4K) {
        KEEP(*(.limine_reqs))
    }

    /* Then place all of the other traditional executable sections afterwards... */
    . = ALIGN(4K);
    .text : {
        __text_start = .;
        *(.text .text.*)
        . = ALIGN(4096);
        __text_end = .;
    }

    . = ALIGN(4K);
    .rodata : {
        __rodata_start = .;
        *(.rodata*)
        . = ALIGN(4096);
        __rodata_end = .;
    }

    . = ALIGN(4K);
    .data : {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(4096);
        __data_end = .;
    }

    . = ALIGN(4K);
    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        . = ALIGN(4096);
        __bss_end = .;
    }
    __kernel_end = .;

    /* Reserve space for stack canary between beginning of stack and end of kernel */
    . += 4096;

    . = ALIGN(4K);
    __stack_start = .;
    . += 16 * 4096;
    __stack_end = .;

    /* Reserve space for stack canary between end of stack and beginning of heap */
    . += 4096;

    . = ALIGN(4K);
    __heap_begin = .;

    __linker_echo = 0xEEEEEEEE;
}