	global _syscall_launch

_syscall_launch:
    push rbp
    mov rbp, rsp

    ;; move arguments around
    mov rdx, rsi
    mov rsi, rdi

    ;; push iretq
    xor rax, rax
    mov ax, ss
    push rax                    ;old ss
    push rbp                    ;old rsp
    pushfq                      ;flags
    mov ax, cs
    push rax                    ;old cs
    push .continue              ;return instruction
    mov rdi, rsp                ;return rsp

    ;; rsi: branch
    ;; rdx: argument
    syscall

    ;; rax: result

.continue:
    pop rbp
    ret
