use x86_64::{structures::paging::PageTable, VirtAddr};
use x86_64::structures::paging::OffsetPageTable;
use x86_64::registers::control::Cr3;
use bootloader::bootinfo::MemoryMap;
use x86_64::{PhysAddr, structures::paging::{PhysFrame, Size4KiB, FrameAllocator}};
use bootloader::bootinfo::MemoryRegionType;

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
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

use alloc::vec;
use alloc::boxed::Box;

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