section .multiboot
align 8
multiboot_header:
    dd 0xE85250D6
    dd 0
    dd multiboot_header_end - multiboot_header
    dd -(0xE85250D6 + 0 + (multiboot_header_end - multiboot_header))

    align 8
    dw 5
    dw 1
    dd 24
    dd 800
    dd 600
    dd 32

    align 8
    dw 0
    dw 0
    dd 8
multiboot_header_end:

section .boot
bits 32
global start
extern rust_main
extern _kernel_end

PML4_TABLE_ADDR   equ 0x10000
PDPT_TABLE_ADDR   equ 0x11000
PDT_TABLE_ADDR    equ 0x12000
PDT1_TABLE_ADDR   equ 0x14000
PDT2_TABLE_ADDR   equ 0x15000
PDT3_TABLE_ADDR   equ 0x16000
GDT64_ADDR        equ 0x13000

start:
    cli
    cld

    mov dword [0x500], eax
    mov dword [0x504], ebx

    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_lm

    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz .no_lm

    call init_paging
    call init_gdt64
    lgdt [gdt64_ptr]

    ; Загружаем TR в 32-битном режиме
    mov ax, 0x18
    ltr ax

    mov eax, PML4_TABLE_ADDR
    mov cr3, eax

    mov eax, cr4
    or eax, 0x20
    mov cr4, eax

    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x100
    wrmsr

    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    jmp 0x08:long_mode_start

.no_lm:
    mov dword [0xB8000], 0x4F4C4F4C
    hlt
    jmp $

init_paging:
    pushad
    mov edi, PML4_TABLE_ADDR
    mov ecx, (4096 * 6) / 4
    xor eax, eax
    rep stosd

    mov eax, PDPT_TABLE_ADDR
    or eax, 0x03
    mov [PML4_TABLE_ADDR], eax

    mov eax, PDT_TABLE_ADDR
    or eax, 0x03
    mov [PDPT_TABLE_ADDR], eax

    mov eax, PDT1_TABLE_ADDR
    or eax, 0x03
    mov [PDPT_TABLE_ADDR + 8], eax

    mov eax, PDT2_TABLE_ADDR
    or eax, 0x03
    mov [PDPT_TABLE_ADDR + 16], eax

    mov eax, PDT3_TABLE_ADDR
    or eax, 0x03
    mov [PDPT_TABLE_ADDR + 24], eax

    %macro fill_pdt 2
        mov edi, %1
        mov ebx, %2
        mov ecx, 512
    %%loop:
        mov [edi], ebx
        add ebx, 0x200000
        add edi, 8
        loop %%loop
    %endmacro

    fill_pdt PDT_TABLE_ADDR,  0x00000083
    fill_pdt PDT1_TABLE_ADDR, 0x40000083
    fill_pdt PDT2_TABLE_ADDR, 0x80000083
    fill_pdt PDT3_TABLE_ADDR, 0xC0000083

    popad
    ret

init_gdt64:
    pushad
    mov edi, GDT64_ADDR
    mov ecx, (104 + 40) / 4
    xor eax, eax
    rep stosd

    mov dword [GDT64_ADDR + 8], 0x0000FFFF
    mov dword [GDT64_ADDR + 12], 0x00209A00

    mov dword [GDT64_ADDR + 16], 0x0000FFFF
    mov dword [GDT64_ADDR + 20], 0x00209200

    ; TSS
    mov eax, GDT64_ADDR + 40
    mov word [GDT64_ADDR + 24], 103
    mov word [GDT64_ADDR + 26], ax
    shr eax, 16
    mov byte [GDT64_ADDR + 28], al
    mov byte [GDT64_ADDR + 29], 0x89
    mov byte [GDT64_ADDR + 30], 0x00
    mov byte [GDT64_ADDR + 31], ah
    mov dword [GDT64_ADDR + 32], 0

    mov word [gdt64_ptr], (4 * 8 - 1)
    mov dword [gdt64_ptr + 2], GDT64_ADDR
    popad
    ret

bits 64
long_mode_start:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    mov rsp, _kernel_end
    add rsp, 0x2000
    and rsp, ~0xF

    ; Заполняем TSS и IOPB
    mov rdi, 0x13028
    mov ecx, 26
    xor eax, eax
    rep stosd
    mov word [0x13028 + 102], 104
    mov byte [0x13028 + 104], 0xFF

    mov edi, dword [0x500]
    mov esi, dword [0x504]

    call rust_main

    cli
    hlt
    jmp $

section .data
gdt64_ptr:
    dw 0
    dd 0
