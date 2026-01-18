.section ".text.exceptions","ax"
.align 11
.global vector_table
.global restore_context
.global start_first
.extern sync_handler

.macro VEC label
  b \label
  .space 0x7c
.endm

vector_table:
  VEC sync_current_sp0
  VEC irq_current_sp0
  VEC fiq_current_sp0
  VEC serr_current_sp0
  VEC sync_current_spx
  VEC irq_current_spx
  VEC fiq_current_spx
  VEC serr_current_spx
  VEC sync_lower_a64
  VEC irq_lower_a64
  VEC fiq_lower_a64
  VEC serr_lower_a64
  VEC sync_lower_a32
  VEC irq_lower_a32
  VEC fiq_lower_a32
  VEC serr_lower_a32

sync_current_sp0: b default_handler
irq_current_sp0:  b default_handler
fiq_current_sp0:  b default_handler
serr_current_sp0: b default_handler
sync_current_spx: b sync_vector
irq_current_spx:  b irq_vector
fiq_current_spx:  b default_handler
serr_current_spx: b default_handler
sync_lower_a64:    b sync_vector
irq_lower_a64:     b irq_vector
fiq_lower_a64:     b default_handler
serr_lower_a64:    b default_handler
sync_lower_a32:    b default_handler
irq_lower_a32:     b default_handler
fiq_lower_a32:     b default_handler
serr_lower_a32:    b default_handler

.default:
default_handler:
  wfe
  b default_handler

// Trap frame size: 288 bytes
.equ TF_SIZE, 0x120

irq_vector:
  sub sp, sp, #TF_SIZE

  stp x0, x1, [sp, #0]
  stp x2, x3, [sp, #16]
  stp x4, x5, [sp, #32]
  stp x6, x7, [sp, #48]
  stp x8, x9, [sp, #64]
  stp x10, x11, [sp, #80]
  stp x12, x13, [sp, #96]
  stp x14, x15, [sp, #112]
  stp x16, x17, [sp, #128]
  stp x18, x19, [sp, #144]
  stp x20, x21, [sp, #160]
  stp x22, x23, [sp, #176]
  stp x24, x25, [sp, #192]
  stp x26, x27, [sp, #208]
  stp x28, x29, [sp, #224]
  stp x30, xzr, [sp, #240]

  mrs x0, elr_el1
  mrs x1, spsr_el1
  mrs x2, sp_el0
  stp x0, x1, [sp, #256]
  str x2, [sp, #272]

  mov x0, sp
  bl irq_handler
  mov sp, x0

  ldp x0, x1, [sp, #256]
  ldr x2, [sp, #272]
  msr elr_el1, x0
  msr spsr_el1, x1
  msr sp_el0, x2

  ldp x30, xzr, [sp, #240]
  ldp x28, x29, [sp, #224]
  ldp x26, x27, [sp, #208]
  ldp x24, x25, [sp, #192]
  ldp x22, x23, [sp, #176]
  ldp x20, x21, [sp, #160]
  ldp x18, x19, [sp, #144]
  ldp x16, x17, [sp, #128]
  ldp x14, x15, [sp, #112]
  ldp x12, x13, [sp, #96]
  ldp x10, x11, [sp, #80]
  ldp x8, x9, [sp, #64]
  ldp x6, x7, [sp, #48]
  ldp x4, x5, [sp, #32]
  ldp x2, x3, [sp, #16]
  ldp x0, x1, [sp, #0]

  add sp, sp, #TF_SIZE
  eret

sync_vector:
  sub sp, sp, #TF_SIZE

  stp x0, x1, [sp, #0]
  stp x2, x3, [sp, #16]
  stp x4, x5, [sp, #32]
  stp x6, x7, [sp, #48]
  stp x8, x9, [sp, #64]
  stp x10, x11, [sp, #80]
  stp x12, x13, [sp, #96]
  stp x14, x15, [sp, #112]
  stp x16, x17, [sp, #128]
  stp x18, x19, [sp, #144]
  stp x20, x21, [sp, #160]
  stp x22, x23, [sp, #176]
  stp x24, x25, [sp, #192]
  stp x26, x27, [sp, #208]
  stp x28, x29, [sp, #224]
  stp x30, xzr, [sp, #240]

  mrs x0, elr_el1
  mrs x1, spsr_el1
  mrs x2, sp_el0
  stp x0, x1, [sp, #256]
  str x2, [sp, #272]

  mov x0, sp
  bl sync_handler
  mov sp, x0

  ldp x0, x1, [sp, #256]
  ldr x2, [sp, #272]
  msr elr_el1, x0
  msr spsr_el1, x1
  msr sp_el0, x2

  ldp x30, xzr, [sp, #240]
  ldp x28, x29, [sp, #224]
  ldp x26, x27, [sp, #208]
  ldp x24, x25, [sp, #192]
  ldp x22, x23, [sp, #176]
  ldp x20, x21, [sp, #160]
  ldp x18, x19, [sp, #144]
  ldp x16, x17, [sp, #128]
  ldp x14, x15, [sp, #112]
  ldp x12, x13, [sp, #96]
  ldp x10, x11, [sp, #80]
  ldp x8, x9, [sp, #64]
  ldp x6, x7, [sp, #48]
  ldp x4, x5, [sp, #32]
  ldp x2, x3, [sp, #16]
  ldp x0, x1, [sp, #0]

  add sp, sp, #TF_SIZE
  eret

restore_context:
  mov sp, x0
  ldp x0, x1, [sp, #256]
  ldr x2, [sp, #272]
  msr elr_el1, x0
  msr spsr_el1, x1
  msr sp_el0, x2

  ldp x30, xzr, [sp, #240]
  ldp x28, x29, [sp, #224]
  ldp x26, x27, [sp, #208]
  ldp x24, x25, [sp, #192]
  ldp x22, x23, [sp, #176]
  ldp x20, x21, [sp, #160]
  ldp x18, x19, [sp, #144]
  ldp x16, x17, [sp, #128]
  ldp x14, x15, [sp, #112]
  ldp x12, x13, [sp, #96]
  ldp x10, x11, [sp, #80]
  ldp x8, x9, [sp, #64]
  ldp x6, x7, [sp, #48]
  ldp x4, x5, [sp, #32]
  ldp x2, x3, [sp, #16]
  ldp x0, x1, [sp, #0]

  add sp, sp, #TF_SIZE
  eret

// x0 = entry, x1 = stack_top
start_first:
  mov sp, x1
  br x0
