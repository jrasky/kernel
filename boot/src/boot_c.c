#include <stddef.h>
#include <stdint.h>

#include "multiboot2.h"

#define ALIGN(x,a)              __ALIGN_MASK(x,(typeof(x))(a)-1)
#define __ALIGN_MASK(x,mask)    (((x)+(mask))&~(mask))

struct multiboot_tag_fixed {
  multiboot_uint32_t total_size;
  multiboot_uint32_t reserved;
  struct multiboot_header_tag tags[];
};

struct module {
  uint64_t start;
  uint64_t len;
  size_t cmdline_size;
  const char *cmdline;
};

struct memory_region {
  uint64_t start;
  uint64_t len;
  uint32_t type;
};

struct boot_info {
  size_t command_line_size;
  const char *command_line;
  size_t memory_map_capacity;
  size_t memory_map_size;
  struct memory_region *memory_map;
  size_t modules_capacity;
  size_t modules_size;
  struct module *modules;
};

extern void *__rust_allocate(size_t size, size_t align);
extern void *__rust_reallocate(void *ptr, size_t old_size, size_t size, size_t align);

const char *error_message = "";

static inline int error(const char *message) {
  error_message = message;
  return -1;
}

static int32_t parse_memory_map(const struct multiboot_tag_mmap *mmap, struct boot_info *kernel_info) {
  // sanity check
  if (mmap->entry_version != 0)
    return error("Unknown boot entry version");

  if (kernel_info->memory_map != NULL)
    return error("More than one memory map entry");

  const struct multiboot_mmap_entry *end = (const struct multiboot_mmap_entry *)((size_t)mmap + (size_t)mmap->size);

  for (const struct multiboot_mmap_entry *entry = mmap->entries; entry < end; entry++) {
    if (kernel_info->memory_map_capacity == 0) {
      kernel_info->memory_map =
        __rust_allocate(4 * sizeof(struct memory_region),
                        sizeof(uint64_t));

      if (kernel_info->memory_map == NULL)
        return error("Failed to allocate memory map");

      kernel_info->memory_map_capacity = 4;
    } else if (kernel_info->memory_map_size == kernel_info->memory_map_capacity) {
      struct memory_region *old_memory_map = kernel_info->memory_map;

      kernel_info->memory_map =
        __rust_reallocate(kernel_info->memory_map, kernel_info->memory_map_capacity * sizeof(struct memory_region),
                          kernel_info->memory_map_capacity * 2 * sizeof(struct memory_region), sizeof(uint64_t));

      if (kernel_info->memory_map == NULL) {
        kernel_info->memory_map = old_memory_map;
        return error("Failed to reallocate memory map");
      }

      kernel_info->memory_map_capacity *= 2;
    }

    kernel_info->memory_map[kernel_info->memory_map_size].start = entry->addr;
    kernel_info->memory_map[kernel_info->memory_map_size].len = entry->len;
    kernel_info->memory_map[kernel_info->memory_map_size].type = entry->type;

    kernel_info->memory_map_size++;
  }

  return 0;
}

static int32_t parse_module(const struct multiboot_tag_module *tag, struct boot_info *kernel_info) {
  if (kernel_info->modules_capacity == 0) {
    kernel_info->modules = __rust_allocate(4 * sizeof(struct module), sizeof(uint64_t));

    if (kernel_info->modules == NULL)
      return error("Failed to allocate modules");

    kernel_info->modules_capacity = 4;
  } else if (kernel_info->modules_size == kernel_info->modules_capacity) {
    struct module *old_modules = kernel_info->modules;

    kernel_info->modules =
      __rust_reallocate(kernel_info->modules, kernel_info->modules_capacity * sizeof(struct module),
                        kernel_info->modules_capacity * 2 * sizeof(struct module), sizeof(uint64_t));

    if (kernel_info->modules == NULL) {
      kernel_info->modules = old_modules;
      return error("Failed to reallocate modules");
    }

    kernel_info->modules_capacity *= 2;
  }

  kernel_info->modules[kernel_info->modules_size].start = tag->mod_start;
  kernel_info->modules[kernel_info->modules_size].len = tag->mod_end - tag->mod_start;
  // null-terminated
  kernel_info->modules[kernel_info->modules_size].cmdline_size = tag->size - sizeof(struct multiboot_tag_module) - 1;
  kernel_info->modules[kernel_info->modules_size].cmdline = tag->cmdline;

  kernel_info->modules_size++;

  return 0;
}

int32_t parse_multiboot_info(const struct multiboot_tag_fixed *info, struct boot_info *kernel_info) {
  const struct multiboot_header_tag *tag = info->tags;

  struct multiboot_tag_string *cmdline;

  while ((size_t)tag < (size_t)info + (size_t)info->total_size) {
    switch (tag->type) {
    case MULTIBOOT_TAG_TYPE_END:
      // end of tags
      goto done;
    case MULTIBOOT_TAG_TYPE_CMDLINE:
      // command line
      cmdline = (struct multiboot_tag_string *)tag;
      // multiboot_tag_string ends with a zero-size char array, so its size is just the header fields
      kernel_info->command_line_size = cmdline->size - sizeof(struct multiboot_tag_string) - 1; // null-terminated
      kernel_info->command_line = cmdline->string;
      break;
    case MULTIBOOT_TAG_TYPE_MMAP:
      // memory map
      if (parse_memory_map((struct multiboot_tag_mmap *)tag, kernel_info) != 0) {
        return -1;
      }

      break;
    case MULTIBOOT_TAG_TYPE_MODULE:
      // module
      if (parse_module((struct multiboot_tag_module *)tag, kernel_info) != 0) {
        return -1;
      }

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
  return 0;
}
