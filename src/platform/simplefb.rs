#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SimpleFbFormat {
    X8R8G8B8,
    A8R8G8B8,
}

#[derive(Copy, Clone, Debug)]
pub struct SimpleFbInfo {
    pub addr: u64,
    pub size: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: SimpleFbFormat,
}
