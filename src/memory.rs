use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{structures::paging::PageTable, VirtAddr};
use x86_64::structures::paging::{OffsetPageTable, PageTableFlags};
use x86_64::registers::control::Cr3;
use bootloader::bootinfo::MemoryMap;
use x86_64::structures::paging::{Page, Mapper};
use x86_64::{PhysAddr, structures::paging::{PhysFrame, Size4KiB, FrameAllocator}};
use bootloader::bootinfo::MemoryRegionType;

lazy_static! {
    static ref PHYSICAL_MEMORY_OFFSET: Mutex<Option<VirtAddr>> = Mutex::new(None);
    static ref FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> = Mutex::new(None);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    NotInitialized,
    OutOfMemory,
    InvalidAddress,
    MappingFailed,
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

// Memory region descriptor
pub struct MemoryRegion {
    start: VirtAddr,
    size: usize, 
    permissions: PageTableFlags,
}

// Memory layout for a process
pub struct ProcessMemoryLayout {
    code_region: MemoryRegion,
    data_region: MemoryRegion,
    heap_region: MemoryRegion, 
    stack_region: MemoryRegion,
    page_table_phys: PhysAddr,
}

pub fn allocate_frame() -> Option<PhysFrame> {
    let mut allocator_guard = FRAME_ALLOCATOR.lock();
    if let Some(ref mut allocator) = *allocator_guard {
        allocator.allocate_frame()
    } else {
        None
    }
}

pub fn create_table_page() -> Result<PhysAddr, MemoryError> {
    let physical_memory_offset = 
    get_physical_memory_offset().ok_or(MemoryError::NotInitialized)?;

    let new_frame = allocate_frame().ok_or(MemoryError::OutOfMemory)?;

    let pml4_virt = physical_memory_offset + new_frame.start_address().as_u64();
    let pml4_table: &mut PageTable = unsafe { &mut *(pml4_virt.as_mut_ptr()) };

    for entry in pml4_table.iter_mut() {
        entry.set_unused();
    }
    
    // Get kernel page table to copy kernel mappings
    let kernel_table = unsafe { active_level_4_table(physical_memory_offset)};
    
    // Copy kernel mappings (entries 256-511 for high half kernel)
    for i in 256..512 {
        pml4_table[i] = kernel_table[i].clone();
    }
    
    Ok(new_frame.start_address())
}

pub fn map_process_memory_region(page_table_phys: PhysAddr,region: &MemoryRegion) -> Result<(), MemoryError> {
    let phys_mem_offset = get_physical_memory_offset()
        .ok_or(MemoryError::NotInitialized)?;
    
    let page_table_virt = phys_mem_offset + page_table_phys.as_u64();
    let page_table: &mut PageTable = unsafe { &mut *(page_table_virt.as_mut_ptr()) };
    let mut mapper = unsafe { OffsetPageTable::new(page_table, phys_mem_offset) };
    
    let start_page = Page::containing_address(region.start);
    let end_page = Page::containing_address(region.start + region.size - 1u64);
    let page_range = Page::range_inclusive(start_page, end_page);
    
    // Map each page in the region
    for page in page_range {
        let frame = allocate_frame().ok_or(MemoryError::OutOfMemory)?;
        
        unsafe {
            mapper.map_to(page, frame, region.permissions, &mut EmptyFrameAllocator)
                .map_err(|_| MemoryError::MappingFailed)?
                .flush();
        }
    }
    
    Ok(())
}


pub fn init_global_memory_management(
    phys_mem_offset: VirtAddr,
    frame_allocator: BootInfoFrameAllocator
) {
    // Store physical memory offset globally
    // Store frame allocator globally  
    // Initialize memory management subsystem
    *PHYSICAL_MEMORY_OFFSET.lock() = Some(phys_mem_offset);
    *FRAME_ALLOCATOR.lock() = Some(frame_allocator);
}

pub fn get_physical_memory_offset() -> Option<VirtAddr> {
    *PHYSICAL_MEMORY_OFFSET.lock()
}

impl MemoryRegion {
    pub fn new(start: VirtAddr, size: usize, permissions: PageTableFlags) -> Self{
        Self {
            start,
            size,
            permissions
        }

    }

    pub fn start_addr(&self) -> VirtAddr {
        self.start
    }
    pub fn end_addr(&self) -> VirtAddr {
        self.start + self.size as u64
    }
    pub fn get_permissions(&self) -> PageTableFlags {
        self.permissions
    }
}

pub fn setup_standard_process_layout(
    page_table_phys: PhysAddr,
    _pid: u64
) -> Result<ProcessMemoryLayout, MemoryError> {
    let code_region = MemoryRegion {
        start: VirtAddr::new(0x0040_0000),
        size: 0x0010_0000,
        permissions: PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
    };
    
    let data_region = MemoryRegion {
        start: VirtAddr::new(0x0080_0000),
        size: 0x0010_0000,
        permissions: PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    };
    
    let heap_region = MemoryRegion {
        start: VirtAddr::new(0x0100_0000),
        size: 0x0040_0000, 
        permissions: PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    };
    
    let stack_region = MemoryRegion {
        start: VirtAddr::new(0x7000_0000),
        size: 0x0001_0000,
        permissions: PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
    };
    
    map_process_memory_region(page_table_phys, &code_region)?;
    map_process_memory_region(page_table_phys, &data_region)?;
    map_process_memory_region(page_table_phys, &heap_region)?;
    map_process_memory_region(page_table_phys, &stack_region)?;
    
    Ok(ProcessMemoryLayout {
        code_region,
        data_region,
        heap_region,
        stack_region,
        page_table_phys,
    })
}

impl ProcessMemoryLayout {
    pub fn new( code_region: MemoryRegion, data_region: MemoryRegion, heap_region: MemoryRegion,
        stack_region: MemoryRegion, page_table_phys: PhysAddr
    ) -> Self {
        Self {
            code_region,
            data_region,
            heap_region,
            stack_region,
            page_table_phys
        }
    }

    pub fn code_start(&self) -> VirtAddr {
        self.code_region.start_addr()
    }

    pub fn data_start(&self) -> VirtAddr {
        self.data_region.start_addr()
    }

    pub fn heap_start(&self) -> VirtAddr {
        self.heap_region.start_addr()
    }

    pub fn stack_start(&self) -> VirtAddr {
        self.stack_region.start_addr()
    }

    pub fn page_table_phys(&self) -> PhysAddr {
        self.page_table_phys
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
        let mut allocator_guard = FRAME_ALLOCATOR.lock();
        if let Some(ref mut allocator) = *allocator_guard {
            allocator.allocate_frame()
        } else {
            None
        }
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