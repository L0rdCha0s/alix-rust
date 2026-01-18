use crate::mm::layout::{align_down, align_up, PAGE_SIZE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionKind {
    UsableRam,
    Reserved,
    Mmio,
    KernelImage,
    BootStack,
    BootInfo,
}

#[derive(Clone, Copy, Debug)]
pub struct PhysRegion {
    pub start: u64,
    pub end: u64,
    pub kind: RegionKind,
}

#[derive(Clone, Copy, Debug)]
pub struct PhysRange {
    pub start: u64,
    pub end: u64,
}

const MAX_REGIONS: usize = 128;

pub struct MemoryMap {
    regions: [PhysRegion; MAX_REGIONS],
    len: usize,
}

impl MemoryMap {
    pub const fn new() -> Self {
        Self {
            regions: [PhysRegion {
                start: 0,
                end: 0,
                kind: RegionKind::Reserved,
            }; MAX_REGIONS],
            len: 0,
        }
    }

    pub fn add_region(&mut self, start: u64, size: u64, kind: RegionKind) {
        // Insert a region without normalization or overlap handling.
        if size == 0 || self.len >= MAX_REGIONS {
            return;
        }
        let end = start.saturating_add(size);
        self.regions[self.len] = PhysRegion { start, end, kind };
        self.len += 1;
    }

    pub fn add_range(&mut self, range: PhysRange, kind: RegionKind) {
        if range.end <= range.start {
            return;
        }
        self.add_region(range.start, range.end - range.start, kind);
    }

    pub fn regions(&self) -> &[PhysRegion] {
        &self.regions[..self.len]
    }

    pub fn normalize(&self) -> NormalizedMap {
        // Convert possibly-overlapping regions into a sorted, non-overlapping map.
        let mut boundaries = [0u64; MAX_REGIONS * 2];
        let mut b_len = 0usize;
        for region in &self.regions[..self.len] {
            boundaries[b_len] = region.start;
            b_len += 1;
            boundaries[b_len] = region.end;
            b_len += 1;
        }
        sort_u64(&mut boundaries[..b_len]);
        let mut out = NormalizedMap::new();
        let mut i = 0;
        while i + 1 < b_len {
            let start = boundaries[i];
            let end = boundaries[i + 1];
            if start == end {
                i += 1;
                continue;
            }
            // Determine the highest-priority region that covers this span.
            let mut kind: Option<RegionKind> = None;
            let mut best_prio = 0u8;
            for region in &self.regions[..self.len] {
                if region.start <= start && region.end >= end {
                    let prio = kind_priority(region.kind);
                    if prio > best_prio {
                        best_prio = prio;
                        kind = Some(region.kind);
                    }
                }
            }
            if let Some(kind) = kind {
                out.push(PhysRegion { start, end, kind });
            }
            i += 1;
        }
        out.merge_adjacent();
        out.align_usable();
        out
    }
}

pub struct NormalizedMap {
    regions: [PhysRegion; MAX_REGIONS],
    len: usize,
}

impl NormalizedMap {
    pub const fn new() -> Self {
        Self {
            regions: [PhysRegion {
                start: 0,
                end: 0,
                kind: RegionKind::Reserved,
            }; MAX_REGIONS],
            len: 0,
        }
    }

    fn push(&mut self, region: PhysRegion) {
        if self.len >= MAX_REGIONS {
            return;
        }
        self.regions[self.len] = region;
        self.len += 1;
    }

    pub fn regions(&self) -> &[PhysRegion] {
        &self.regions[..self.len]
    }

    pub fn usable_regions(&self) -> impl Iterator<Item = PhysRange> + '_ {
        self.regions[..self.len]
            .iter()
            .filter(|r| r.kind == RegionKind::UsableRam)
            .map(|r| PhysRange {
                start: r.start,
                end: r.end,
            })
    }

    pub fn max_phys_end(&self) -> u64 {
        self.regions[..self.len]
            .iter()
            .map(|r| r.end)
            .max()
            .unwrap_or(0)
    }

    fn merge_adjacent(&mut self) {
        // Merge back-to-back regions of the same kind.
        if self.len == 0 {
            return;
        }
        let mut out = [PhysRegion {
            start: 0,
            end: 0,
            kind: RegionKind::Reserved,
        }; MAX_REGIONS];
        let mut out_len = 0usize;
        let mut current = self.regions[0];
        for region in &self.regions[1..self.len] {
            if region.kind == current.kind && region.start == current.end {
                current.end = region.end;
            } else {
                out[out_len] = current;
                out_len += 1;
                current = *region;
            }
        }
        out[out_len] = current;
        out_len += 1;
        self.regions = out;
        self.len = out_len;
    }

    fn align_usable(&mut self) {
        // Page-align usable RAM ranges to avoid partial frames.
        for region in &mut self.regions[..self.len] {
            if region.kind != RegionKind::UsableRam {
                continue;
            }
            let start = align_up(region.start, PAGE_SIZE as u64);
            let end = align_down(region.end, PAGE_SIZE as u64);
            if end <= start {
                region.start = 0;
                region.end = 0;
                region.kind = RegionKind::Reserved;
            } else {
                region.start = start;
                region.end = end;
            }
        }
    }
}

fn kind_priority(kind: RegionKind) -> u8 {
    match kind {
        RegionKind::KernelImage => 6,
        RegionKind::BootStack => 5,
        RegionKind::BootInfo => 4,
        RegionKind::Reserved => 3,
        RegionKind::Mmio => 2,
        RegionKind::UsableRam => 1,
    }
}

fn sort_u64(values: &mut [u64]) {
    let len = values.len();
    let mut i = 0;
    while i < len {
        let mut j = i + 1;
        while j < len {
            if values[j] < values[i] {
                values.swap(i, j);
            }
            j += 1;
        }
        i += 1;
    }
}
