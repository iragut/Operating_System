use core::cell::UnsafeCell;

pub struct RamFsCell(UnsafeCell<RamFs>);

unsafe impl Sync for RamFsCell {}
unsafe impl Send for RamFsCell {}

impl RamFsCell {
    const fn new() -> Self {
        RamFsCell(UnsafeCell::new(RamFs::new()))
    }

    pub unsafe fn get(&self) -> &mut RamFs {
        &mut *self.0.get()
    }
}

pub static RAMFS: RamFsCell = RamFsCell::new();

pub struct RamFs {
    entries: [Option<FileEntry>; 16],
    count: usize,
}

pub struct FileEntry {
    pub name: [u8; 32],
    pub name_len: usize,
    pub data: &'static [u8],
}

impl FileEntry {
    pub const fn new(name: [u8; 32], name_len: usize, data: &'static [u8]) -> Self {
        FileEntry {
            name,
            name_len,
            data,
        }
    }
}

impl RamFs {
    pub const fn new() -> Self {
        RamFs {
            entries: [const { None }; 16],
            count: 0,
        }
    }

    pub fn add(&mut self, name: &str, data: &'static [u8]) {
        let mut name_buf = [0u8; 32];
        let len = name.len().min(32);
        name_buf[..len].copy_from_slice(&name.as_bytes()[..len]);

        self.entries[self.count] = Some(FileEntry::new(name_buf, len, data));
        self.count += 1;
    }

    pub fn find(&self, name: &str) -> Option<&FileEntry> {
        for entry in &self.entries {
            if let Some(entry) = entry {
                if &entry.name[..entry.name_len] == name.as_bytes() {
                    return Some(entry);
                }
            }
        }
        None
    }
}