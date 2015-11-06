/*
 * kernel.c
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
