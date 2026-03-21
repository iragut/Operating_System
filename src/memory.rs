use x86_64::{structures::paging::PageTable, VirtAddr};
 use x86_64::structures::paging::page_table::PageTableEntry;
use x86_64::structures::paging::OffsetPageTable;
use x86_64::structures::paging::PageTableFlags;
use x86_64::registers::control::Cr3;
use core::cell::UnsafeCell;
use bootloader::bootinfo::MemoryMap;
use x86_64::{PhysAddr, structures::paging::{PhysFrame, Size4KiB, FrameAllocator}};
use bootloader::bootinfo::MemoryRegionType;
use alloc::vec;

use alloc::boxed::Box;

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

pub struct FrameAllocatorCell(UnsafeCell<Option<BootInfoFrameAllocator>>);


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

unsafe impl Sync for FrameAllocatorCell {}
unsafe impl Send for FrameAllocatorCell {}

impl FrameAllocatorCell {
    const fn new() -> Self {
        FrameAllocatorCell(UnsafeCell::new(None))
    }

    pub unsafe fn get(&self) -> &mut BootInfoFrameAllocator {
        (*self.0.get()).as_mut().expect("Frame allocator not initialized")
    }

    pub unsafe fn init(&self, alloc: BootInfoFrameAllocator) {
        *self.0.get() = Some(alloc);
    }
}

pub static FRAME_ALLOCATOR: FrameAllocatorCell = FrameAllocatorCell::new();
pub static mut PHYS_MEM_OFFSET: u64 = 0;

pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

pub fn allocate_kernel_stack() -> VirtAddr {
    // Allocate stack on the heap
    const STACK_SIZE: usize = 8192; // 8KB stack
    
    // Create a boxed array for the stack
    let stack_vec = vec![0u8; STACK_SIZE];
    let stack_box = stack_vec.into_boxed_slice();
    
    // Leak the box to get a static lifetime (we don't want it freed)
    let stack_bottom = Box::leak(stack_box).as_ptr();
    
    // Return the TOP of the stack
    let stack_top = unsafe {
        VirtAddr::from_ptr(stack_bottom.add(STACK_SIZE))
    };
    
    stack_top
}

pub fn create_process_page_table(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    phys_mem_offset: VirtAddr,
) -> PhysFrame {

    // Allocate a new frame for the process's page table
    let phy_frame = frame_allocator.allocate_frame().expect("No more frames available");

    let phy_addr = phy_frame.start_address();
    let virt_addr = phys_mem_offset + phy_addr.as_u64();

    // Zero it
    let page_table_ptr: *mut PageTable = virt_addr.as_mut_ptr();
    unsafe {
        page_table_ptr.write(PageTable::new());
    };

    // Copy the kernel to the new page table
    let kernel_table = unsafe { active_level_4_table(phys_mem_offset) };
    let new_table = unsafe { &mut *page_table_ptr };

    for i in 0..512 {
        new_table[i] = kernel_table[i].clone();
    }

    phy_frame
}

fn get_or_create_table(entry: &mut PageTableEntry, phys_mem_offset: VirtAddr,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> PhysFrame {
    if entry.is_unused() {
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        let frame = frame_allocator.allocate_frame().expect("No more frames");

        let table_ptr = (phys_mem_offset + frame.start_address().as_u64()).as_mut_ptr::<PageTable>();
        unsafe { table_ptr.write(PageTable::new()) };

        entry.set_addr(frame.start_address(), flags);
        frame
    } else {
        let flags = entry.flags() | PageTableFlags::USER_ACCESSIBLE;
        entry.set_flags(flags);
        PhysFrame::containing_address(entry.addr())
    }
}


pub fn map_user_page(page_table_frame: PhysFrame,phys_mem_offset: VirtAddr,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    virt_addr: VirtAddr, flags: PageTableFlags,
) -> PhysFrame {

    // Get table reference from a physical frame
    let table = |frame: PhysFrame| -> &mut PageTable {
        unsafe { &mut *(phys_mem_offset + frame.start_address().as_u64()).as_mut_ptr::<PageTable>() }
    };

    // PML4 -> PDPT
    let pml4 = table(page_table_frame);
    let pdpt_frame = get_or_create_table(&mut pml4[virt_addr.p4_index()], phys_mem_offset, frame_allocator);

    // PDPT -> PD
    let pdpt = table(pdpt_frame);
    let pd_frame = get_or_create_table(&mut pdpt[virt_addr.p3_index()], phys_mem_offset, frame_allocator);

    // PD -> PT
    let pd = table(pd_frame);
    let pt_frame = get_or_create_table(&mut pd[virt_addr.p2_index()], phys_mem_offset, frame_allocator);

    // PT
    let pt = table(pt_frame);
    let data_frame = frame_allocator.allocate_frame().expect("No more frames");
    pt[virt_addr.p1_index()].set_addr(data_frame.start_address(), flags);

    data_frame
    
}