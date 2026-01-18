# Drivers

## UART
- `src/drivers/uart.rs`
- Used for early boot logging and debug output.

## Mailbox
- `src/drivers/mailbox.rs`
- Provides property channel access for framebuffer setup.

## Framebuffer
- `src/drivers/framebuffer.rs`
- Initializes framebuffer via mailbox and provides a scrolling text console.

## Keyboard
- `src/drivers/keyboard.rs`
- PS/2 input via polling.

## Local interrupt controller
- `src/drivers/local_intc.rs`
- Routes per-core interrupts for the generic timer.
