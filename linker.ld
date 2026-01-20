ENTRY(_start)

KERNEL_PHYS_BASE = 0x80000;
KERNEL_VIRT_BASE = 0xFFFF800000000000;

SECTIONS
{
  /* Low boot stub (runs before MMU is enabled). */
  . = KERNEL_PHYS_BASE;
  .boot : ALIGN(16)
  {
    KEEP(*(.boot))
  }
  .boot.bss (NOLOAD) : ALIGN(16)
  {
    *(.boot.bss)
  }
  __boot_phys_end = ALIGN(., 0x200000);

  /* Higher-half kernel mapping. */
  __kernel_start = KERNEL_VIRT_BASE + __boot_phys_end;

  .text __kernel_start : AT(__boot_phys_end)
  {
    *(.text .text.*)
  }

  . = ALIGN(16);
  .rodata : AT(((LOADADDR(.text) + SIZEOF(.text) + 15) & ~15))
  {
    *(.rodata .rodata.*)
  }

  . = ALIGN(16);
  .data : AT(((LOADADDR(.rodata) + SIZEOF(.rodata) + 15) & ~15))
  {
    *(.data .data.*)
    . = ALIGN(8);
    __secondary_start_ptr = .;
    QUAD(secondary_start);
    . = ALIGN(8);
    __kernel_phys_info = .;
    QUAD(LOADADDR(.text)); /* kernel_start_phys */
    QUAD(LOADADDR(.bss) + SIZEOF(.bss) + 0x10000); /* kernel_end_phys */
    QUAD(LOADADDR(.bss) + SIZEOF(.bss)); /* stack_start_phys */
    QUAD(LOADADDR(.bss) + SIZEOF(.bss) + 0x10000); /* stack_end_phys */
    QUAD(ABSOLUTE(__boot_phys_end)); /* boot_end_phys */
  }

  . = ALIGN(4096);
  .bss (NOLOAD) : AT(((LOADADDR(.data) + SIZEOF(.data) + 4095) & ~4095))
  {
    __bss_start = .;
    *(.bss .bss.* COMMON)
    __bss_end = .;
  }

  . = ALIGN(16);
  __stack_start = .;
  . = . + 0x10000; /* 64 KiB */
  __stack_end = .;

  __kernel_end = .;

  /* Physical aliases for early boot use. */
  __kernel_virt_base = KERNEL_VIRT_BASE;
  __boot_phys_start = KERNEL_PHYS_BASE;
  __kernel_start_phys = LOADADDR(.text);
  __bss_start_phys = LOADADDR(.bss);
  __bss_end_phys = LOADADDR(.bss) + SIZEOF(.bss);
  __stack_start_phys = __bss_end_phys;
  __stack_end_phys = __bss_end_phys + 0x10000;
  __kernel_end_phys = __stack_end_phys;

  /DISCARD/ : { *(.eh_frame .comment) }
}
