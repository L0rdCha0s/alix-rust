ENTRY(_start)

SECTIONS
{
  . = 0x80000;

  .text : ALIGN(16)
  {
    KEEP(*(.text._start))
    *(.text .text.*)
  }

  .rodata : ALIGN(16)
  {
    *(.rodata .rodata.*)
  }

  .data : ALIGN(16)
  {
    *(.data .data.*)
  }

  .bss (NOLOAD) : ALIGN(16)
  {
    __bss_start = .;
    *(.bss .bss.* COMMON)
    __bss_end = .;
  }

  . = ALIGN(16);
  __stack_start = .;
  . = . + 0x10000; /* 64 KiB */
  __stack_end = .;

  /DISCARD/ : { *(.eh_frame .comment) }
}
