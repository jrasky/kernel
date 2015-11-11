;;; long.asm
;;;
;;; Copyright (C) 2015 Jerome Rasky
;;; 
;;; Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
;;; http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
;;; or http://opensource.org/licenses/MIT>, at your option. This file may not be
;;; copied, modified, or distributed except according to those terms.

    global _lstart

    extern kmain
    extern _bss

    section .text
    bits 64
_lstart:
    ;; In long mode!

    call kmain

_hang:
    cli
    hlt
    jmp _hang
