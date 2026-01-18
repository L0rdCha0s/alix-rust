# Graphics

## Overview
The framebuffer console uses a fixed-width font to render text.

## Key files
- src/gfx/font.rs
- src/drivers/framebuffer.rs

## Notes
- Font is 8x16 logical pixels (rendered as 8x16 with vertical doubling).
- Console scrolls when it reaches the bottom of the screen.
