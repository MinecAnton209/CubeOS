core::arch::global_asm!(
    r#"
    .global context_switch
    .type context_switch, @function
    context_switch:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    pushfq
    mov [rdi], rsp
    mov rsp, rsi
    popfq
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    test rdx, rdx
    jz 1f
    mov cr3, rdx
    1:
    ret
    .size context_switch, . - context_switch
    "#
);

extern "C" {
    pub fn context_switch(save_rsp: *mut u64, load_rsp: u64, next_cr3: u64);
}
