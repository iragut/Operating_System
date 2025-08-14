use x86_64::{structures::paging::PageTable, VirtAddr};
use x86_64::structures::paging::{OffsetPageTable, PageTableFlags};
use x86_64::registers::control::Cr3;
use bootloader::bootinfo::MemoryMap;
use x86_64::{PhysAddr, structures::paging::{PhysFrame, Size4KiB, FrameAllocator}};
use bootloader::bootinfo::MemoryRegionType;

#[derive(Debug)]
pub enum MemoryError {
    AllocationFailed,
    InvalidAddress,
    PermissionDenied,
    PageTableError,
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

#[derive(Clone, Copy)]
pub struct MemoryRegion {
    start: VirtAddr,
    size: usize, 
    permissions: PageTableFlags,
}

// Memory layout for a process
#[derive(Clone, Copy)]
pub struct ProcessMemoryLayout {
    code_region: MemoryRegion,
    data_region: MemoryRegion,
    heap_region: MemoryRegion, 
    stack_region: MemoryRegion,
    page_table_phys: PhysAddr,
}

impl MemoryRegion {
    pub fn new(start: VirtAddr, size: usize, permissions: PageTableFlags) -> Self {
        Self { start, size, permissions }
    }
    
    pub fn end_address(&self) -> VirtAddr {
        self.start + self.size
    }
    
    pub fn contains_address(&self, addr: VirtAddr) -> bool {
        addr >= self.start && addr < self.end_address()
    }
    
    pub fn is_writable(&self) -> bool {
        self.permissions.contains(PageTableFlags::WRITABLE)
    }
    
    pub fn is_executable(&self) -> bool {
        !self.permissions.contains(PageTableFlags::NO_EXECUTE)
    }
}


impl ProcessMemoryLayout {
    pub fn create_simple_layout(entry_point: VirtAddr) -> Result<Self, MemoryError> {
        const CODE_BASE: u64 = 0x0000_0000_0040_0000;   // 4MB
        const DATA_BASE: u64 = 0x0000_0000_0080_0000;   // 8MB
        const HEAP_BASE: u64 = 0x0000_0000_0100_0000;   // 16MB  
        const STACK_BASE: u64 = 0x0000_7fff_ffff_0000;  // High address
        
        const PAGE_SIZE: usize = 4096;
        const STACK_SIZE: usize = PAGE_SIZE * 4; // 16KB stack
        
        // Create memory regions
        let code_region = MemoryRegion::new(
            VirtAddr::new(CODE_BASE),
            PAGE_SIZE * 4, // 16KB for code
            PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE
        );
        
        let data_region = MemoryRegion::new(
            VirtAddr::new(DATA_BASE),
            PAGE_SIZE * 4,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE
        );
        
        let heap_region = MemoryRegion::new(
            VirtAddr::new(HEAP_BASE),
            PAGE_SIZE * 16,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE
        );
        
        let stack_region = MemoryRegion::new(
            VirtAddr::new(STACK_BASE - STACK_SIZE as u64),
            STACK_SIZE,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE
        );
        
        // TODO: Actually create and set up page table
        let page_table_phys = PhysAddr::new(0); // Placeholder for now
        
        Ok(Self {
            code_region,
            data_region, 
            heap_region,
            stack_region,
            page_table_phys,
        })
    }
}


unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
}

impl BootInfoFrameAllocator {
    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}
