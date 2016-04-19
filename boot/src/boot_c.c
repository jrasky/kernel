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
  const struct module *modules;
};

struct module {
  uint64_t start;
  uint64_t len;
  const char *cmdline;
}

extern void *__rust_allocate(size_t size, size_t align);
extern void *__rust_reallocate(void *ptr, size_t old_size, size_t size, size_t align);
extern void boot_c_panic(const char *message) __attribute__((noreturn));

uint64_t find_heap(const struct multiboot_tag_mmap *mmap) {
  for (multiboot_memory_map_t *entry = mmap->entries;
       (size_t)entry < (size_t)mmap + (size_t)mmap->size;
       entry++) {
    if (entry->addr >= 0x100000 && && entry->len >= 0x200000 &&
        entry->type == MULTIBOOT_MEMORY_AVAILABLE) {
      return entry->addr;
    }
  }
  
  return 0;
}

struct boot_info *parse_multiboot_info(const struct multiboot_tag_fixed *info) {
  struct boot_info *kernel_info = __rust_allocate(sizeof(struct boot_info), sizeof(size_t));
  size_t modules_size = 0;
  size_t modules_cap = 0;

  if (kernel_info == NULL)
    boot_c_panic("Failed to allocate kernel boot info");

  // zero-initialize area
  memset(kernel_info, 0, sizeof(struct boot_info));

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
      kernel_info->memory_map = mmap->entries;
      break;
    case MULTIBOOT_TAG_TYPE_MODULE:
      // module
      struct multiboot_tag_module *module_tag = (struct multiboot_tag_module *)tag;

      if (modules_cap == 0) {
        modules_cap = 4;
        kernel_info->modules = __rust_allocate(modules_cap * sizeof(struct module), sizeof(uint64_t));

        if (modules == NULL)
          boot_c_panic("Failed to allocate modules");
      } else if (modules_size == modules_cap) {
        struct module *old_modules = modules;

        kernel_info->modules =
          __rust_reallocate(modules, modules_cap * sizeof(struct module),
                            modules_cap * 2 * sizeof(struct module), sizeof(uint64_t));

        if (modules == old_modules)
          boot_c_panic("Failed to reallocate modules");
      }

      kernel_info->modules[modules_size].start = module_tag->mod_start;
      kernel_info->modules[modules_size].len = module_tag->mod_end - module_tag->mod_start;
      kernel_info->modules[modules_size].cmdline = module_tag->cmdline;

      modules_size++;
      break;
    default:
      // do nothing
      break;
    }

    // advance tag
    size_t next = (size_t)tag + tag->size;
    tag = (struct multiboot_header_tag *)ALIGN(next, MULTIBOOT_TAG_ALIGN);
  }

 done:
  return kernel_info;
}
