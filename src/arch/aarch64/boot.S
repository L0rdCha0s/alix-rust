.section ".boot","ax"
.global _start
.global secondary_start

.extern __secondary_table
.extern __secondary_stacks
.extern secondary_rust_entry
.extern vector_table
.extern kernel_main
.extern __bss_start_phys
.extern __bss_end_phys
.extern __stack_end_phys
.extern __kernel_virt_base

// Boot page tables (identity + higher-half alias)
.section ".boot.bss","aw",%nobits
.align 12
boot_l0:
  .skip 4096
.align 12
boot_l1:
  .skip 4096
.align 12
boot_l2:
  .skip 4096
.align 12
boot_l2_rp1:
  .skip 4096

.section ".boot","ax"

_start:
  // Preserve boot arg (DTB pointer in x0). On RPi, x0 is a 32-bit address;
  // the upper bits may be garbage, so zero-extend it.
  mov w20, w0
  // Drop to EL1 if needed.
  mrs x0, CurrentEL
  lsr x0, x0, #2
  cmp x0, #3
  b.eq el3_to_el1
  cmp x0, #2
  b.eq el2_to_el1
  b el1_start

el3_to_el1:
  // Non-secure, AArch64.
  mov x1, #(1 << 10) | 1
  msr scr_el3, x1

  // Allow FP/SIMD usage in lower ELs.
  msr cptr_el3, xzr

  // Enable EL1 access to the physical timer.
  mov x1, #0x3
  msr cnthctl_el2, x1
  msr cntvoff_el2, xzr

  // Mask interrupts on entry to EL1.
  mov x1, #0x3C5
  msr spsr_el3, x1

  adr x1, el1_start
  msr elr_el3, x1
  eret

el2_to_el1:
  // Enable EL1 access to the physical timer.
  mov x1, #0x3
  msr cnthctl_el2, x1
  msr cntvoff_el2, xzr

  // Allow FP/SIMD usage in EL1.
  msr cptr_el2, xzr

  // Set EL1 to AArch64.
  mov x1, #(1 << 31)
  msr hcr_el2, x1

  // Mask interrupts on entry to EL1.
  mov x1, #0x3C5
  msr spsr_el2, x1

  adr x1, el1_start
  msr elr_el2, x1
  eret

el1_start:
  // Ensure MMU/caches are off in case firmware left them enabled.
  mrs x0, sctlr_el1
  bic x0, x0, #(1 << 0)   // M
  bic x0, x0, #(1 << 2)   // C
  bic x0, x0, #(1 << 12)  // I
  msr sctlr_el1, x0
  isb

  // Enable FP/SIMD in EL1 (compiler may use SIMD regs).
  mov x1, #(3 << 20)
  msr cpacr_el1, x1
  isb

  // Use SP_EL1.
  mov x1, #1
  msr spsel, x1

  mrs x0, mpidr_el1
  and x0, x0, #3
  cbz x0, 2f

1:
  // Secondary cores wait for release (physical address).
  ldr x3, =__kernel_virt_base
  ldr x1, =__secondary_table
  sub x1, x1, x3
  ldr x2, [x1, x0, lsl #3]
  cbz x2, 3f
  b secondary_el1_setup
3:
  wfe
  b 1b

2:
  // Set up a simple physical stack.
  ldr x1, =__stack_end_phys
  mov sp, x1

  // Early proof-of-life: write markers to RP1 UART0 (PL011) if enabled.
  ldr x21, =0x1c00030000      // RP1 UART0 base (phys)
  mov w1, #'B'
  str w1, [x21, #0x00]
  mov w1, #'\r'
  str w1, [x21, #0x00]
  mov w1, #'\n'
  str w1, [x21, #0x00]

  // Zero BSS using physical addresses.
  ldr x1, =__bss_start_phys
  ldr x2, =__bss_end_phys
3:
  cmp x1, x2
  b.hs 4f
  str xzr, [x1], #8
  b 3b

4:
  mov w1, #'b'
  str w1, [x21, #0x00]
  mov w1, #'\r'
  str w1, [x21, #0x00]
  mov w1, #'\n'
  str w1, [x21, #0x00]

  // Build boot page tables and enable the MMU.
  bl boot_enable_mmu
  mov w1, #'m'
  str w1, [x21, #0x00]
  mov w1, #'\r'
  str w1, [x21, #0x00]
  mov w1, #'\n'
  str w1, [x21, #0x00]

  // Install exception vectors (now valid at higher-half VA).
  ldr x1, =vector_table
  msr vbar_el1, x1
  isb
  mov w1, #'v'
  str w1, [x21, #0x00]
  mov w1, #'\r'
  str w1, [x21, #0x00]
  mov w1, #'\n'
  str w1, [x21, #0x00]

  // Switch to higher-half kernel stack.
  ldr x1, =__stack_end
  mov sp, x1
  mov w1, #'s'
  str w1, [x21, #0x00]
  mov w1, #'\r'
  str w1, [x21, #0x00]
  mov w1, #'\n'
  str w1, [x21, #0x00]

  mov x0, x20
  mov w2, #'k'
  str w2, [x21, #0x00]
  mov w2, #'\r'
  str w2, [x21, #0x00]
  mov w2, #'\n'
  str w2, [x21, #0x00]
  ldr x1, =kernel_main
  br x1

5:
  wfe
  b 5b

// Secondary entry point set by core0.
secondary_start:
  // Drop secondary cores to EL1 if needed.
  mrs x0, CurrentEL
  lsr x0, x0, #2
  cmp x0, #3
  b.eq secondary_el3_to_el1
  cmp x0, #2
  b.eq secondary_el2_to_el1
  b secondary_el1_setup

secondary_el3_to_el1:
  // Non-secure, AArch64.
  mov x1, #(1 << 10) | 1
  msr scr_el3, x1

  // Allow FP/SIMD usage in lower ELs.
  msr cptr_el3, xzr

  // Enable EL1 access to the physical timer.
  mov x1, #0x3
  msr cnthctl_el2, x1
  msr cntvoff_el2, xzr

  // Mask interrupts on entry to EL1.
  mov x1, #0x3C5
  msr spsr_el3, x1

  adr x1, secondary_el1_setup
  msr elr_el3, x1
  eret

secondary_el2_to_el1:
  // Enable EL1 access to the physical timer.
  mov x1, #0x3
  msr cnthctl_el2, x1
  msr cntvoff_el2, xzr

  // Allow FP/SIMD usage in EL1.
  msr cptr_el2, xzr

  // Set EL1 to AArch64.
  mov x1, #(1 << 31)
  msr hcr_el2, x1

  // Mask interrupts on entry to EL1.
  mov x1, #0x3C5
  msr spsr_el2, x1

  adr x1, secondary_el1_setup
  msr elr_el2, x1
  eret

secondary_el1_setup:
  // Enable FP/SIMD in EL1 (secondary cores skip el1_start).
  mov x1, #(3 << 20)
  msr cpacr_el1, x1
  isb

  // Use SP_EL1.
  mov x1, #1
  msr spsel, x1

  // Load core id.
  mrs x0, mpidr_el1
  and x0, x0, #3
  mov x19, x0

  // Set up a physical stack for this core before MMU.
  ldr x3, =__kernel_virt_base
  ldr x1, =__secondary_stacks
  sub x1, x1, x3
  mov x2, #0x4000
  mul x4, x19, x2
  add x1, x1, x4
  add sp, x1, x2

  // Enable MMU using the boot tables.
  bl boot_enable_mmu

  // Install exception vectors (now valid at higher-half VA).
  ldr x1, =vector_table
  msr vbar_el1, x1
  isb

  // Switch to higher-half stack for this core.
  ldr x1, =__secondary_stacks
  mov x2, #0x4000
  mul x4, x19, x2
  add x1, x1, x4
  add sp, x1, x2

  ldr x1, =secondary_rust_entry
  br x1

6:
  wfe
  b 6b

// Build a minimal identity + higher-half alias mapping and enable the MMU.
boot_enable_mmu:
  // Zero L0/L1/L2 tables.
  adr x0, boot_l0
  mov x1, #0
  mov x2, #512
1:
  str x1, [x0], #8
  subs x2, x2, #1
  b.ne 1b

  adr x0, boot_l1
  mov x2, #512
2:
  str x1, [x0], #8
  subs x2, x2, #1
  b.ne 2b

  adr x0, boot_l2
  mov x2, #512
3:
  str x1, [x0], #8
  subs x2, x2, #1
  b.ne 3b

  // L0[0] = L1 table, L0[256] = L1 table (higher-half alias).
  adr x0, boot_l0
  adr x1, boot_l1
  orr x2, x1, #0x3
  str x2, [x0]
  mov x3, #256
  str x2, [x0, x3, lsl #3]

  // L1[0] = L2 table.
  adr x0, boot_l1
  adr x1, boot_l2
  orr x2, x1, #0x3
  str x2, [x0]

  // L1[0x70] = RP1 MMIO table (maps 0x1c00000000..0x1c3fffffff).
  adr x1, boot_l2_rp1
  orr x2, x1, #0x3
  mov x3, #0x70
  str x2, [x0, x3, lsl #3]

  // Fill L2 with 2 MiB blocks (identity for 0..1GiB).
  adr x0, boot_l2
  mov x3, #0
  mov x4, #512
4:
  lsl x5, x3, #21
  // Normal memory flags: attr1 + SH=inner + AF + block.
  mov x6, #0x705
  // Map MMIO range (0x3F000000..0x40000000) as device.
  cmp x3, #0x1F8
  b.lt 5f
  mov x6, #0x401
5:
  orr x6, x6, x5
  str x6, [x0], #8
  add x3, x3, #1
  subs x4, x4, #1
  b.ne 4b

  // Fill RP1 L2 with device blocks.
  adr x0, boot_l2_rp1
  mov x3, #0
  mov x4, #512
  mov x7, #0x1c
  lsl x7, x7, #32
6:
  lsl x5, x3, #21
  add x5, x7, x5
  mov x6, #0x401
  orr x6, x6, x5
  str x6, [x0], #8
  add x3, x3, #1
  subs x4, x4, #1
  b.ne 6b

  // MAIR: attr0=device, attr1=normal WBWA.
  mov x0, #0xff
  lsl x0, x0, #8
  msr mair_el1, x0

  // TCR for 4 KiB granule, 48-bit VA, 40-bit PA.
  // T0SZ=16, T1SZ=16, IRGN/ORGN=WBWA, SH=inner, TG0=4K, TG1=4K.
  mov x0, #16
  orr x0, x0, #(16 << 16)
  orr x0, x0, #(1 << 8)
  orr x0, x0, #(1 << 10)
  orr x0, x0, #(3 << 12)
  // TG0 = 0 (4K)
  orr x0, x0, #(1 << 24)
  orr x0, x0, #(1 << 26)
  orr x0, x0, #(3 << 28)
  // TG1 = 0b10 (64K)
  orr x0, x0, #(2 << 30)
  // IPS = 0b010 (40-bit)
  orr x0, x0, #(2 << 32)
  msr tcr_el1, x0
  isb

  adr x1, boot_l0
  msr ttbr0_el1, x1
  msr ttbr1_el1, x1
  dsb ish
  isb

  // Enable MMU (caches off for now).
  mrs x0, sctlr_el1
  bic x0, x0, #(1 << 0)
  bic x0, x0, #(1 << 2)
  bic x0, x0, #(1 << 12)
  msr sctlr_el1, x0
  isb
  orr x0, x0, #(1 << 0)
  msr sctlr_el1, x0
  isb
  ret
