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

    section .multiboot2
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
    dd 20                     ; size
    dd 8                      ; framebuffer info
    dd 9                      ; elf info
    dd 0                      ; end info requests

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

;; Framebuffer tag
    dw 5, 0
    dd 20
    dd 0
    dd 0
    dd 0

    align GNUALIGN

;; End tag
    dw 0, 0                 ; type, flags
    dd 8

;; end label
multiboot2_header_end:   
