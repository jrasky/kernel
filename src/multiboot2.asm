;;; multiboot2.S
;;;
;;; Copyright (C) 2015 Jerome Rasky
;;; 
;;; Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
;;; http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
;;; or http://opensource.org/licenses/MIT>, at your option. This file may not be
;;; copied, modified, or distributed except according to those terms.
    
;; Declare constants
    GNUALIGN equ 8
    MAGIC equ 0xE85250D6
    ARCH equ 0
    LENGTH equ multiboot2_header_end - multiboot2_header_top
    CHECKSUM equ 0xFFFFFFFF & -(MAGIC + ARCH + LENGTH)

    section .text
    align GNUALIGN

;; top label
multiboot2_header_top:

;; header
    dd MAGIC
    dd ARCH
    dd LENGTH
    dd CHECKSUM

    align GNUALIGN

;; Info request tag
    dw 1, 0
    dd 16                     ; size
    dd 8                      ; framebuffer info
    dd 0                      ; end info requests

    align GNUALIGN

;; Address tag
    dw 2, 0                 ; type, flags
    dd 24                    ; size
    dd multiboot2_header_top ; header address
    extern _kernel_top
    dd _kernel_top           ; start of kernel .text
    extern _kernel_end
    dd _kernel_end           ; end of kernel .text
    extern _stack_top
    dd _stack_top            ; end of uninitialized data, including stack

    align GNUALIGN

;; Entry tag
    dw 3, 0                 ; type, flags
    dd 12                    ; size
    extern _start
    dd _start                ; entry address

    align GNUALIGN

;; Flags tag
    dw 4, 0
    dd 12
    dd 3

    align GNUALIGN

;; End tag
    dw 0, 0                 ; type, flags
    dd 8

;; end label
multiboot2_header_end:   
