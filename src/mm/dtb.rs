use crate::mm::region::{MemoryMap, RegionKind};
use crate::platform::simplefb::{SimpleFbFormat, SimpleFbInfo};

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

#[derive(Copy, Clone)]
struct SimpleFbState {
    is_simplefb: bool,
    addr: u64,
    size: u64,
    width: u32,
    height: u32,
    stride: u32,
    format: Option<SimpleFbFormat>,
}

#[derive(Copy, Clone, Debug)]
pub struct UartInfo {
    pub addr: u64,
    pub size: u64,
    pub reg_shift: u32,
    pub reg_io_width: u32,
    pub clock_hz: Option<u32>,
    pub skip_init: bool,
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

pub fn find_simplefb(dtb_pa: u64) -> Option<SimpleFbInfo> {
    if dtb_pa == 0 {
        return None;
    }
    let base = dtb_pa as *const u8;
    let header = unsafe { core::slice::from_raw_parts(base, 40) };
    let magic = read_be_u32(&header[0..4]);
    if magic != FDT_MAGIC {
        return None;
    }
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
    let mut fb_stack: [SimpleFbState; 32] = [SimpleFbState {
        is_simplefb: false,
        addr: 0,
        size: 0,
        width: 0,
        height: 0,
        stride: 0,
        format: None,
    }; 32];
    let mut depth = 0usize;

    while offset + 4 <= struct_block.len() {
        let token = read_be_u32(&struct_block[offset..offset + 4]);
        offset += 4;
        match token {
            FDT_BEGIN_NODE => {
                // Enter a new node and inherit address/size cell defaults.
                while offset < struct_block.len() && struct_block[offset] != 0 {
                    offset += 1;
                }
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
                let ctx = parent;
                if depth < stack.len() {
                    stack[depth] = ctx;
                    fb_stack[depth] = SimpleFbState {
                        is_simplefb: false,
                        addr: 0,
                        size: 0,
                        width: 0,
                        height: 0,
                        stride: 0,
                        format: None,
                    };
                    depth += 1;
                }
            }
            FDT_END_NODE => {
                if depth > 0 {
                    let idx = depth - 1;
                    let fb = fb_stack[idx];
                    if fb.is_simplefb
                        && fb.addr != 0
                        && fb.width != 0
                        && fb.height != 0
                        && fb.stride != 0
                    {
                        if let Some(format) = fb.format {
                            return Some(SimpleFbInfo {
                                addr: fb.addr,
                                size: fb.size,
                                width: fb.width,
                                height: fb.height,
                                stride: fb.stride,
                                format,
                            });
                        }
                    }
                    depth -= 1;
                }
            }
            FDT_PROP => {
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
                let fb = &mut fb_stack[depth - 1];
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
                    b"compatible" => {
                        if value_has_string(value, b"simple-framebuffer") {
                            fb.is_simplefb = true;
                        }
                    }
                    b"reg" => {
                        let tuple_cells = (ctx.addr_cells + ctx.size_cells) as usize;
                        if tuple_cells == 0 {
                            continue;
                        }
                        let entry_bytes = tuple_cells * 4;
                        if value.len() < entry_bytes {
                            continue;
                        }
                        let addr = read_cells(&value[..ctx.addr_cells as usize * 4], ctx.addr_cells);
                        let size = read_cells(
                            &value[ctx.addr_cells as usize * 4..entry_bytes],
                            ctx.size_cells,
                        );
                        fb.addr = addr;
                        fb.size = size;
                    }
                    b"width" => {
                        if len >= 4 {
                            fb.width = read_be_u32(value);
                        }
                    }
                    b"height" => {
                        if len >= 4 {
                            fb.height = read_be_u32(value);
                        }
                    }
                    b"stride" => {
                        if len >= 4 {
                            fb.stride = read_be_u32(value);
                        }
                    }
                    b"format" => {
                        fb.format = parse_format(value);
                    }
                    _ => {}
                }
            }
            FDT_NOP => {}
            FDT_END => break,
            _ => break,
        }
    }

    None
}

pub fn find_uart(dtb_pa: u64) -> Option<UartInfo> {
    if dtb_pa == 0 {
        return None;
    }

    let mut stdout = SmallBuf::new();
    scan_stdout_path(dtb_pa, &mut stdout);

    let mut target = SmallBuf::new();
    let mut alias = SmallBuf::new();

    // Prefer serial0 (GPIO UART on Pi 5) when available.
    if read_alias_path(dtb_pa, b"serial0", &mut target) {
        // target set.
    } else if stdout.len != 0 {
        if stdout.buf[0] == b'/' {
            target = stdout;
        } else {
            alias = stdout;
        }
    }

    if target.len == 0 && alias.len != 0 {
        let _ = read_alias_path(dtb_pa, alias.as_slice(), &mut target);
    }

    if target.len == 0 {
        return None;
    }

    find_reg_by_path(dtb_pa, target.as_slice())
}

#[derive(Copy, Clone)]
struct Range {
    child_base: u64,
    parent_base: u64,
    size: u64,
}

#[derive(Copy, Clone)]
struct RegNode {
    addr_cells: u32,
    size_cells: u32,
    ranges: [Range; 4],
    ranges_len: usize,
}

#[derive(Copy, Clone)]
struct SmallBuf {
    buf: [u8; 256],
    len: usize,
}

impl SmallBuf {
    const fn new() -> Self {
        Self {
            buf: [0; 256],
            len: 0,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn set_from(&mut self, src: &[u8], stop: Option<u8>) {
        self.len = 0;
        for &b in src.iter() {
            if b == 0 {
                break;
            }
            if let Some(stop_byte) = stop {
                if b == stop_byte {
                    break;
                }
            }
            if self.len >= self.buf.len() {
                break;
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

fn scan_stdout_path(dtb_pa: u64, out: &mut SmallBuf) {
    out.clear();
    let base = dtb_pa as *const u8;
    let header = unsafe { core::slice::from_raw_parts(base, 40) };
    let magic = read_be_u32(&header[0..4]);
    if magic != FDT_MAGIC {
        return;
    }
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
    let mut depth = 0usize;
    let mut in_chosen = false;
    let mut chosen_depth = 0usize;

    while offset + 4 <= struct_block.len() {
        let token = read_be_u32(&struct_block[offset..offset + 4]);
        offset += 4;
        match token {
            FDT_BEGIN_NODE => {
                let name_start = offset;
                while offset < struct_block.len() && struct_block[offset] != 0 {
                    offset += 1;
                }
                let name = &struct_block[name_start..offset];
                offset = align4(offset + 1);
                let is_root = depth == 0 && name.is_empty();
                depth += 1;
                if !is_root && depth == 2 && name == b"chosen" {
                    in_chosen = true;
                    chosen_depth = depth;
                }
            }
            FDT_END_NODE => {
                if in_chosen && depth == chosen_depth {
                    in_chosen = false;
                }
                if depth > 0 {
                    depth -= 1;
                }
            }
            FDT_PROP => {
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
                if !in_chosen {
                    continue;
                }
                let name = get_string(strings_block, nameoff);
                if name == b"stdout-path" {
                    out.set_from(value, Some(b':'));
                    return;
                }
            }
            FDT_NOP => {}
            FDT_END => break,
            _ => break,
        }
    }
}

fn read_alias_path(dtb_pa: u64, alias: &[u8], out: &mut SmallBuf) -> bool {
    out.clear();
    let base = dtb_pa as *const u8;
    let header = unsafe { core::slice::from_raw_parts(base, 40) };
    let magic = read_be_u32(&header[0..4]);
    if magic != FDT_MAGIC {
        return false;
    }
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
    let mut depth = 0usize;
    let mut in_aliases = false;
    let mut aliases_depth = 0usize;

    while offset + 4 <= struct_block.len() {
        let token = read_be_u32(&struct_block[offset..offset + 4]);
        offset += 4;
        match token {
            FDT_BEGIN_NODE => {
                let name_start = offset;
                while offset < struct_block.len() && struct_block[offset] != 0 {
                    offset += 1;
                }
                let name = &struct_block[name_start..offset];
                offset = align4(offset + 1);
                let is_root = depth == 0 && name.is_empty();
                depth += 1;
                if !is_root && depth == 2 && name == b"aliases" {
                    in_aliases = true;
                    aliases_depth = depth;
                }
            }
            FDT_END_NODE => {
                if in_aliases && depth == aliases_depth {
                    in_aliases = false;
                }
                if depth > 0 {
                    depth -= 1;
                }
            }
            FDT_PROP => {
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
                if !in_aliases {
                    continue;
                }
                let name = get_string(strings_block, nameoff);
                if name == alias {
                    out.set_from(value, None);
                    return true;
                }
            }
            FDT_NOP => {}
            FDT_END => break,
            _ => break,
        }
    }
    false
}

fn find_reg_by_path(dtb_pa: u64, target: &[u8]) -> Option<UartInfo> {
    let base = dtb_pa as *const u8;
    let header = unsafe { core::slice::from_raw_parts(base, 40) };
    let magic = read_be_u32(&header[0..4]);
    if magic != FDT_MAGIC {
        return None;
    }
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
    let mut depth = 0usize;
    let mut stack: [RegNode; 32] = [RegNode {
        addr_cells: 2,
        size_cells: 2,
        ranges: [Range {
            child_base: 0,
            parent_base: 0,
            size: 0,
        }; 4],
        ranges_len: 0,
    }; 32];
    let mut path = SmallBuf::new();
    let mut path_len_stack: [usize; 32] = [0; 32];

    let mut reg_shift = 0u32;
    let mut reg_io_width = 4u32;
    let mut clock_hz: Option<u32> = None;
    let mut skip_init = false;

    while offset + 4 <= struct_block.len() {
        let token = read_be_u32(&struct_block[offset..offset + 4]);
        offset += 4;
        match token {
            FDT_BEGIN_NODE => {
                let name_start = offset;
                while offset < struct_block.len() && struct_block[offset] != 0 {
                    offset += 1;
                }
                let name = &struct_block[name_start..offset];
                offset = align4(offset + 1);
                if depth < path_len_stack.len() {
                    path_len_stack[depth] = path.len;
                }
                let is_root = depth == 0 && name.is_empty();
                if !is_root {
                    if path.len < path.buf.len() {
                        path.buf[path.len] = b'/';
                        path.len += 1;
                    }
                    for &b in name.iter() {
                        if path.len >= path.buf.len() {
                            break;
                        }
                        path.buf[path.len] = b;
                        path.len += 1;
                    }
                }

                let parent = if depth == 0 {
                    RegNode {
                        addr_cells: 2,
                        size_cells: 2,
                        ranges: [Range {
                            child_base: 0,
                            parent_base: 0,
                            size: 0,
                        }; 4],
                        ranges_len: 0,
                    }
                } else {
                    stack[depth - 1]
                };
                let mut ctx = parent;
                ctx.ranges_len = 0;
                if depth < stack.len() {
                    stack[depth] = ctx;
                    depth += 1;
                }
            }
            FDT_END_NODE => {
                if depth > 0 {
                    depth -= 1;
                    path.len = path_len_stack[depth];
                }
            }
            FDT_PROP => {
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
                if depth == 0 {
                    continue;
                }
                let name = get_string(strings_block, nameoff);
                let (addr_cells, size_cells) = {
                    let c = &stack[depth - 1];
                    (c.addr_cells, c.size_cells)
                };
                let parent_addr_cells = if depth >= 2 {
                    stack[depth - 2].addr_cells
                } else {
                    0
                };
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
                    b"ranges" => {
                        let tuple_cells = (addr_cells + parent_addr_cells + size_cells) as usize;
                        if tuple_cells == 0 {
                            continue;
                        }
                        let entry_bytes = tuple_cells * 4;
                        let mut pos = 0usize;
                        ctx.ranges_len = 0;
                        while pos + entry_bytes <= value.len() && ctx.ranges_len < ctx.ranges.len() {
                            let child_base = read_addr_cells(
                                &value[pos..pos + addr_cells as usize * 4],
                                addr_cells,
                            );
                            let parent_base = read_cells_trunc(
                                &value[pos + addr_cells as usize * 4
                                    ..pos + (addr_cells + parent_addr_cells) as usize * 4],
                                parent_addr_cells,
                            );
                            let size = read_cells(
                                &value[pos + (addr_cells + parent_addr_cells) as usize * 4
                                    ..pos + entry_bytes],
                                size_cells,
                            );
                            ctx.ranges[ctx.ranges_len] = Range {
                                child_base,
                                parent_base,
                                size,
                            };
                            ctx.ranges_len += 1;
                            pos += entry_bytes;
                        }
                    }
                    b"reg-shift" => {
                        if path_matches(&path, target) && len >= 4 {
                            reg_shift = read_be_u32(value);
                        }
                    }
                    b"reg-io-width" => {
                        if path_matches(&path, target) && len >= 4 {
                            reg_io_width = read_be_u32(value);
                        }
                    }
                    b"clock-frequency" => {
                        if path_matches(&path, target) && len >= 4 {
                            clock_hz = Some(read_be_u32(value));
                        }
                    }
                    b"skip-init" => {
                        if path_matches(&path, target) {
                            skip_init = true;
                        }
                    }
                    b"reg" => {
                        if !path_matches(&path, target) {
                            continue;
                        }
                        let tuple_cells = (ctx.addr_cells + ctx.size_cells) as usize;
                        if tuple_cells == 0 {
                            continue;
                        }
                        let entry_bytes = tuple_cells * 4;
                        if value.len() < entry_bytes {
                            continue;
                        }
                        let addr =
                            read_addr_cells(&value[..ctx.addr_cells as usize * 4], ctx.addr_cells);
                        let size = read_cells(
                            &value[ctx.addr_cells as usize * 4..entry_bytes],
                            ctx.size_cells,
                        );
                        let mut phys = translate_addr(addr, depth, &stack);
                        if phys.is_none() && is_rp1_path(&path) {
                            phys = Some(rp1_fixup(addr));
                        }
                        let mut phys = phys?;
                        if is_rp1_path(&path) && phys < 0x1_0000_0000 {
                            phys = rp1_fixup(addr);
                        }
                        return Some(UartInfo {
                            addr: phys,
                            size,
                            reg_shift,
                            reg_io_width,
                            clock_hz,
                            skip_init,
                        });
                    }
                    _ => {}
                }
            }
            FDT_NOP => {}
            FDT_END => break,
            _ => break,
        }
    }

    None
}

fn path_matches(path: &SmallBuf, target: &[u8]) -> bool {
    path.len == target.len() && &path.buf[..path.len] == target
}

fn is_rp1_path(path: &SmallBuf) -> bool {
    const PREFIX: &[u8] = b"/axi/pcie@1000120000/rp1/";
    path.len >= PREFIX.len() && &path.buf[..PREFIX.len()] == PREFIX
}

fn rp1_fixup(addr: u64) -> u64 {
    const RP1_CHILD_BASE: u64 = 0x0000_00C0_4000_0000;
    const RP1_PHYS_BASE: u64 = 0x0000_001C_0000_0000;
    if addr >= RP1_CHILD_BASE {
        RP1_PHYS_BASE + (addr - RP1_CHILD_BASE)
    } else {
        RP1_PHYS_BASE + addr
    }
}

fn translate_addr(addr: u64, depth: usize, stack: &[RegNode; 32]) -> Option<u64> {
    let mut cur = addr;
    if depth == 0 {
        return Some(cur);
    }
    let mut idx = depth - 1;
    while idx > 0 {
        let parent = &stack[idx - 1];
        if parent.ranges_len > 0 {
            let mut mapped = None;
            for i in 0..parent.ranges_len {
                let range = parent.ranges[i];
                if cur >= range.child_base && cur < range.child_base.saturating_add(range.size) {
                    let delta = cur - range.child_base;
                    mapped = Some(range.parent_base + delta);
                    break;
                }
            }
            if let Some(next) = mapped {
                cur = next;
            } else {
                return None;
            }
        }
        idx -= 1;
    }
    Some(cur)
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

fn read_cells_trunc(buf: &[u8], cells: u32) -> u64 {
    if cells <= 2 {
        return read_cells(buf, cells);
    }
    let start = (cells as usize).saturating_sub(2) * 4;
    if start >= buf.len() {
        return 0;
    }
    read_cells(&buf[start..], 2)
}

fn read_addr_cells(buf: &[u8], cells: u32) -> u64 {
    if cells > 2 {
        read_cells_trunc(buf, cells)
    } else {
        read_cells(buf, cells)
    }
}

fn name_starts_with(name: &[u8], prefix: &[u8]) -> bool {
    if name.len() < prefix.len() {
        return false;
    }
    &name[..prefix.len()] == prefix
}

fn value_has_string(buf: &[u8], needle: &[u8]) -> bool {
    let mut start = 0usize;
    while start < buf.len() {
        let mut end = start;
        while end < buf.len() && buf[end] != 0 {
            end += 1;
        }
        if end > start && &buf[start..end] == needle {
            return true;
        }
        start = end + 1;
    }
    false
}

fn parse_format(buf: &[u8]) -> Option<SimpleFbFormat> {
    let mut end = 0usize;
    while end < buf.len() && buf[end] != 0 {
        end += 1;
    }
    let s = &buf[..end];
    match s {
        b"x8r8g8b8" => Some(SimpleFbFormat::X8R8G8B8),
        b"a8r8g8b8" => Some(SimpleFbFormat::A8R8G8B8),
        _ => None,
    }
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
