use crate::mm::region::{MemoryMap, RegionKind};

const FDT_MAGIC: u32 = 0xD00D_FEED;

const FDT_BEGIN_NODE: u32 = 0x1;
const FDT_END_NODE: u32 = 0x2;
const FDT_PROP: u32 = 0x3;
const FDT_NOP: u32 = 0x4;
const FDT_END: u32 = 0x9;

#[derive(Copy, Clone)]
pub struct DtbInfo {
    pub total_size: u32,
}

#[derive(Copy, Clone)]
struct NodeContext {
    addr_cells: u32,
    size_cells: u32,
    in_reserved: bool,
    is_memory: bool,
}

pub fn parse(dtb_pa: u64, map: &mut MemoryMap) -> Option<DtbInfo> {
    // Parse a flattened device tree (DTB) into memory regions.
    if dtb_pa == 0 {
        return None;
    }
    let base = dtb_pa as *const u8;
    let header = unsafe { core::slice::from_raw_parts(base, 40) };
    let magic = read_be_u32(&header[0..4]);
    if magic != FDT_MAGIC {
        return None;
    }
    let total_size = read_be_u32(&header[4..8]);
    let off_dt_struct = read_be_u32(&header[8..12]) as usize;
    let off_dt_strings = read_be_u32(&header[12..16]) as usize;
    let size_dt_struct = read_be_u32(&header[36..40]) as usize;
    let size_dt_strings = read_be_u32(&header[32..36]) as usize;

    let struct_block = unsafe {
        core::slice::from_raw_parts(base.add(off_dt_struct), size_dt_struct)
    };
    let strings_block = unsafe {
        core::slice::from_raw_parts(base.add(off_dt_strings), size_dt_strings)
    };

    let mut offset = 0usize;
    let mut stack: [NodeContext; 32] = [NodeContext {
        addr_cells: 2,
        size_cells: 2,
        in_reserved: false,
        is_memory: false,
    }; 32];
    let mut depth = 0usize;

    while offset + 4 <= struct_block.len() {
        let token = read_be_u32(&struct_block[offset..offset + 4]);
        offset += 4;
        match token {
            FDT_BEGIN_NODE => {
                // Enter a new node and inherit address/size cell defaults.
                let name_start = offset;
                while offset < struct_block.len() && struct_block[offset] != 0 {
                    offset += 1;
                }
                let name = &struct_block[name_start..offset];
                offset = align4(offset + 1);
                let parent = if depth == 0 {
                    NodeContext {
                        addr_cells: 2,
                        size_cells: 2,
                        in_reserved: false,
                        is_memory: false,
                    }
                } else {
                    stack[depth - 1]
                };
                let mut ctx = parent;
                if name_starts_with(name, b"reserved-memory") {
                    ctx.in_reserved = true;
                }
                if name_starts_with(name, b"memory") {
                    ctx.is_memory = true;
                }
                if depth < stack.len() {
                    stack[depth] = ctx;
                    depth += 1;
                }
            }
            FDT_END_NODE => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            FDT_PROP => {
                // Parse properties of the current node.
                if offset + 8 > struct_block.len() {
                    break;
                }
                let len = read_be_u32(&struct_block[offset..offset + 4]) as usize;
                let nameoff = read_be_u32(&struct_block[offset + 4..offset + 8]) as usize;
                offset += 8;
                if offset + len > struct_block.len() {
                    break;
                }
                let value = &struct_block[offset..offset + len];
                offset = align4(offset + len);
                let name = get_string(strings_block, nameoff);
                if depth == 0 {
                    continue;
                }
                let ctx = &mut stack[depth - 1];
                match name {
                    b"#address-cells" => {
                        if len >= 4 {
                            ctx.addr_cells = read_be_u32(value);
                        }
                    }
                    b"#size-cells" => {
                        if len >= 4 {
                            ctx.size_cells = read_be_u32(value);
                        }
                    }
                    b"device_type" => {
                        if name_starts_with(value, b"memory") {
                            ctx.is_memory = true;
                        }
                    }
                    b"reg" => {
                        // Parse address/size tuples in the reg property.
                        let tuple_cells = (ctx.addr_cells + ctx.size_cells) as usize;
                        if tuple_cells == 0 {
                            continue;
                        }
                        let entry_bytes = tuple_cells * 4;
                        let mut pos = 0usize;
                        while pos + entry_bytes <= value.len() {
                            let addr = read_cells(&value[pos..pos + ctx.addr_cells as usize * 4], ctx.addr_cells);
                            let size = read_cells(
                                &value[pos + ctx.addr_cells as usize * 4..pos + entry_bytes],
                                ctx.size_cells,
                            );
                            if ctx.is_memory {
                                // Memory nodes provide usable RAM ranges.
                                map.add_region(addr, size, RegionKind::UsableRam);
                            } else if ctx.in_reserved {
                                // Reserved-memory nodes are excluded from allocation.
                                map.add_region(addr, size, RegionKind::Reserved);
                            }
                            pos += entry_bytes;
                        }
                    }
                    _ => {}
                }
            }
            FDT_NOP => {}
            FDT_END => break,
            _ => break,
        }
    }

    Some(DtbInfo { total_size })
}

fn read_cells(buf: &[u8], cells: u32) -> u64 {
    let mut value = 0u64;
    let mut i = 0;
    while i < cells as usize {
        let start = i * 4;
        if start + 4 > buf.len() {
            break;
        }
        value = (value << 32) | read_be_u32(&buf[start..start + 4]) as u64;
        i += 1;
    }
    value
}

fn name_starts_with(name: &[u8], prefix: &[u8]) -> bool {
    if name.len() < prefix.len() {
        return false;
    }
    &name[..prefix.len()] == prefix
}

fn get_string(strings: &[u8], offset: usize) -> &[u8] {
    if offset >= strings.len() {
        return &[];
    }
    let mut end = offset;
    while end < strings.len() && strings[end] != 0 {
        end += 1;
    }
    &strings[offset..end]
}

fn read_be_u32(buf: &[u8]) -> u32 {
    ((buf[0] as u32) << 24)
        | ((buf[1] as u32) << 16)
        | ((buf[2] as u32) << 8)
        | (buf[3] as u32)
}

fn align4(value: usize) -> usize {
    (value + 3) & !3
}
