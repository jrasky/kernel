#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "multiboot2.h"

#define ALIGN(x,a)              __ALIGN_MASK(x,(typeof(x))(a)-1)
#define __ALIGN_MASK(x,mask)    (((x)+(mask))&~(mask))

struct multiboot_tag_fixed {
  multiboot_uint32_t total_size;
  multiboot_uint32_t reserved;
  struct multiboot_header_tag tags[];
};

struct boot_info {
  size_t command_line_size;
  const char *command_line;
  size_t memory_map_size;
  const multiboot_memory_map_t *memory_map;
};

extern void *__rust_allocate(size_t size, size_t align);
extern void boot_c_panic(const char *message) __attribute__((noreturn));

struct boot_info *parse_multiboot_info(struct multiboot_tag_fixed *info) {
  struct boot_info *kernel_info = __rust_allocate(sizeof(struct boot_info), sizeof(size_t));

  if (kernel_info == NULL)
    boot_c_panic("Failed to allocate kernel boot info");

  struct multiboot_header_tag tag = info->tags;

  while ((size_t)tag < (size_t)info + (size_t)info->total_size) {
    switch (tag->type) {
    case MULTIBOOT_TAG_TYPE_END:
      // end of tags
      goto done;
    case MULTIBOOT_TAG_TYPE_CMDLINE:
      // command line
      struct multiboot_tag_string *cmdline = (struct multiboot_tag_string *)tag;
      // multiboot_tag_string ends with a zero-size char array, so its size is just the header fields
      kernel_info->command_line_size = cmdline->size - sizeof(struct multiboot_tag_string);
      kernel_info->command_line = cmdline->string;
      break;
    case MULTIBOOT_TAG_TYPE_MMAP:
      // memory map
      struct multiboot_tag_mmap *mmap = (struct multiboot_tag_mmap *)tag;

      // sanity check
      if (mmap->entry_version != 0)
        boot_c_panic("Unknown boot entry version");

      kernel_info->memory_map_size = mmap->size - sizeof(struct multiboot_tag_mmap);
      kernel_info->memory_map = __rust_allocate(size, sizeof(uint64_t));
      break;
    default:
      // do nothing
      break;
    }

    // advance tag
    size_t next = (size_t)tag + tag->size;
    tag = (struct multiboot_header_tag *)ALIGN(tag, MULTIBOOT_TAG_ALIGN);
  }

 done:
  return kernel_info;
}
