ENTRY(start)

SECTIONS {
    . = 1M;

    .multiboot ALIGN(8) : {
        KEEP(*(.multiboot))
    }

    .boot ALIGN(4K) : {
        *(.boot)
    }

    .text ALIGN(4K) : {
        *(.text .text.*)
    }

    .rodata ALIGN(4K) : {
        *(.rodata .rodata.*)
    }

    .data ALIGN(4K) : {
        *(.data .data.*)
    }

    .bss ALIGN(4K) : {
        *(COMMON)
        *(.bss .bss.*)
    }

    /* PROGRAM_BUF — отдельно от .bss, чтобы не затирать IDT/TSS */
    .program_buf ALIGN(4K) : {
        KEEP(*(.program_buf))
    }

    /* KERNEL_STACK — отдельно, вершина стека для TSS.rsp0 */
    .kernel_stack ALIGN(4K) : {
        KEEP(*(.kernel_stack))
    }

    /* Зазор под RSP ядра (растёт вниз от _kernel_end + 0x3F000) */
    . = ALIGN(4K);
    . += 0x40000;   /* 256 KB */

    _kernel_end = .;
}
