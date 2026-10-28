OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
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

    .stack (NOLOAD) : ALIGN(16) {
        _stack_bottom = .;
        . += 0x40000;
        _stack_top = .;
    }

    _kernel_end = .;
}
