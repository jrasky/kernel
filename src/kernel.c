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
#define GNU_ALIGN 8

struct boot_info_header;
struct boot_info_tag_header;
struct boot_info_framebuffer;

typedef struct boot_info_header boot_info_header_t;
typedef struct boot_info_tag_header boot_info_tag_header_t;
typedef struct boot_info_framebuffer boot_info_framebuffer_t;

void kmain(void);
//void kmain(uint32_t addr);
void draw_screen(boot_info_framebuffer_t *);

struct boot_info_tag_header {
  uint32_t type;
  uint32_t size;
  struct {} _data[];
};

struct boot_info_header {
  uint32_t total_size;
  uint32_t reserved;
  boot_info_tag_header_t tags[];
};

struct boot_info_framebuffer {
  uint32_t type;
  uint32_t size;
  uint64_t framebuffer_addr;
  uint32_t framebuffer_pitch;
  uint32_t framebuffer_width;
  uint32_t framebuffer_height;
  uint8_t framebuffer_bpp;
  uint8_t framebuffer_type;
  uint8_t reserved;
  union {
    struct {
      uint32_t framebuffer_palette_number_colors;
      struct {
        uint8_t red_value;
        uint8_t green_value;
        uint8_t blue_value;
      } framebuffer_palette[];
    } indexed;

    struct {
      uint8_t framebuffer_red_field_position;
      uint8_t framebuffer_red_mask_size;
      uint8_t framebuffer_green_field_position;
      uint8_t framebuffer_green_mask_size;
      uint8_t framebuffer_blue_field_position;
      uint8_t framebuffer_blue_mask_size;
    } direct;

    struct {
      // nothing
    } text;
  };
};

static inline void outb(uint16_t port, uint8_t val) {
    asm volatile ( "outb %0, %1" : : "a"(val), "Nd"(port) );
    /* TODO: Is it wrong to use 'N' for the port? It's not a 8-bit constant. */
    /* TODO: Should %1 be %w1? */
}

static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    asm volatile ( "inb %1, %0" : "=a"(ret) : "Nd"(port) );
    /* TODO: Is it wrong to use 'N' for the port? It's not a 8-bit constant. */
    /* TODO: Should %1 be %w1? */
    return ret;
}

static inline void io_wait(void) {
    /* Port 0x80 is used for 'checkpoints' during POST. */
    /* The Linux kernel seems to think it is free for use :-/ */
    asm volatile ( "outb %%al, $0x80" : : "a"(0) );
    /* TODO: Is there any reason why al is forced? */
}

// align pointer to given bytes
static inline void *align(void *ptr, size_t to) {
  return (void *)(((size_t)ptr + to - 1) & ~(to - 1));
}

static const char str[] = "Hello!";
static const char mb1[] = "Booted from Multiboot";
static const char mb2[] = "Booted from Multiboot 2";
static const char fbinfo[] = "Got framebuffer info";
static char *vidptr = (char*)0xb8000; // video memory begins here

boot_info_header_t *boot_info;

/*void kmain(boot_info_header_t *boot_info) {
  // write welcome string and clear screen
  for (size_t i = 0; i < TERM_ROWS * TERM_LINES; i++) {
    if (i < sizeof(str) - 1) {
      // write the character from the string
      vidptr[i * 2] = str[i];
    } else {
      // clear the character
      vidptr[i * 2] = ' ';
    }

    // set the color
    vidptr[i * 2 + 1] = 0x07;
  }

  for (size_t i = TERM_ROWS; i - TERM_ROWS < sizeof(mb2); i++) {
    // write the character from the string
    vidptr[i * 2] = mb2[i - TERM_ROWS];
  }

  // boot info address is guarenteed to be 8-bytes aligned
  // as well, the boot info header is of length 64, which is also eight bytes
  // alligned: this means we don't have to align this address
  boot_info_tag_header_t *tag = boot_info->tags;
  boot_info_tag_header_t *tags_end = tag + boot_info->total_size;

  // ensure we don't read past the boot info
  while (tag <= tags_end) {
    if (tag->type == 8) {
      // framebuffer info
      draw_screen((boot_info_framebuffer_t *)tag);
      break;
    }

    // advance to the next tag
    tag = align((void *)((size_t)tag + tag->size), GNU_ALIGN);
  }
}

void draw_screen(boot_info_framebuffer_t *info) {
  // we have framebuffer info!
  // quick hackey thing to test output, assuming we have a text framebuffer
  char *ptr = (char *)info->framebuffer_addr;
  for (size_t i = TERM_ROWS * 2; i - TERM_ROWS * 2 < sizeof(fbinfo); i++) {
    // write the character from the string
    ptr[i * 2] = fbinfo[i - TERM_ROWS * 2];
  }
}*/

void kmain(void) {
  // write welcome string and clear screen
  for (size_t i = 0; i < TERM_ROWS * TERM_LINES; i++) {
    if (i < sizeof(str) - 1) {
      // write the character from the string
      vidptr[i * 2] = str[i];
    } else {
      // clear the character
      vidptr[i * 2] = ' ';
    }

    // set the color
    vidptr[i * 2 + 1] = 0x07;
  }

  /*
  size_t acc = 1;

  size_t temp = (size_t)boot_info;
  char *ptr = vidptr + (TERM_ROWS * acc * 2 - 2);
  if (temp == 0) {
    *ptr = '0';
  }
  while (temp > 0) {
    *ptr = '0' + (temp % 10);
    temp /= 10;
    ptr -= 2;
  }
  acc++;

  // boot info address is guarenteed to be 8-bytes aligned
  // as well, the boot info header is of length 64, which is also eight bytes
  // alligned: this means we don't have to align this address
  boot_info_tag_header_t *tag = boot_info->tags;
  boot_info_tag_header_t *tags_end = (boot_info_tag_header_t *)((size_t)tag + boot_info->total_size);

  temp = (size_t)boot_info->total_size;
  ptr = vidptr + (TERM_ROWS * acc * 2 - 2);
  if (temp == 0) {
    *ptr = '0';
  }
  while (temp > 0) {
    *ptr = '0' + (temp % 10);
    temp /= 10;
    ptr -= 2;
  }
  acc++;

  // ensure we don't read past the boot info
  while (tag <= tags_end) {
    size_t temp = (size_t)tag;
    char *ptr = vidptr + (TERM_ROWS * acc * 2 - 2);
    if (temp == 0) {
      *ptr = '0';
    }
    while (temp > 0) {
      *ptr = '0' + (temp % 10);
      temp /= 10;
      ptr -= 2;
    }
    acc++;
    if (tag->type == 8) {
      // framebuffer info
      draw_screen((boot_info_framebuffer_t *)tag);
      break;
    }

    // advance to the next tag
    tag = align((void *)((size_t)tag + tag->size), GNU_ALIGN);
  }*/
}

