/*
 * kernel.c
 *
 * Copyright (C) 2015 Jerome Rasky
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
 * or http://opensource.org/licenses/MIT>, at your option. This file may not be
 * copied, modified, or distributed except according to those terms.
 */
#include <stddef.h>
#include <stdint.h>

#define TERM_ROWS 80
#define TERM_LINES 25

static const char str[] = "my first kernel";
static const size_t str_size = sizeof(str) - 1;
static char *vidptr = (char*)0xb8000; // video memory begins here

void kmain(void) {
  // write str and clear the screen
  for (size_t i = 0; i < TERM_ROWS * TERM_LINES; i++) {
    if (i < str_size) {
      // write the character from the string
      vidptr[i * 2] = str[i];
    } else {
      // clear the character
      vidptr[i * 2] = ' ';
    }

    // set the color
    vidptr[i * 2 + 1] = 0x07;
  }
}
