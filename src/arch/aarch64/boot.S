.section ".text._start","ax"
.global _start
.global secondary_start

.extern __secondary_table
.extern __secondary_stacks
.extern secondary_rust_entry
.extern vector_table

_start:
  // Preserve boot arg (DTB pointer in x0) before clobbering.
  mov x20, x0
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
  // Enable FP/SIMD in EL1 (compiler may use SIMD regs).
  mov x1, #(3 << 20)
  msr cpacr_el1, x1
  isb

  // Install exception vectors.
  ldr x1, =vector_table
  msr vbar_el1, x1
  isb
  // Use SP_EL1.
  mov x1, #1
  msr spsel, x1

  mrs x0, mpidr_el1
  and x0, x0, #3
  cbz x0, 2f

1:
  // Secondary cores wait for release.
  adrp x1, __secondary_table
  add x1, x1, :lo12:__secondary_table
  ldr x2, [x1, x0, lsl #3]
  cbz x2, 3f
  br x2
3:
  wfe
  b 1b

2:
  // Set up a simple stack.
  ldr x1, =__stack_end
  mov sp, x1

  // Zero BSS.
  ldr x1, =__bss_start
  ldr x2, =__bss_end
3:
  cmp x1, x2
  b.hs 4f
  str xzr, [x1], #8
  b 3b

4:
  mov x0, x20
  bl kernel_main

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

  // Install exception vectors for secondary cores.
  ldr x1, =vector_table
  msr vbar_el1, x1
  isb

  // Use SP_EL1.
  mov x1, #1
  msr spsel, x1

  // Load core id for stack selection.
  mrs x0, mpidr_el1
  and x0, x0, #3
  adrp x1, __secondary_stacks
  add x1, x1, :lo12:__secondary_stacks
  mov x2, #0x4000
  mul x3, x0, x2
  add x1, x1, x3
  add sp, x1, x2

  bl secondary_rust_entry

6:
  wfe
  b 6b
