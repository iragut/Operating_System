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
use x86_64::structures::paging::{Page, Mapper, PageTableFlags};

pub const KERNEL_STACK_SIZE: usize = 8192; // 8KB
pub const USER_STACK_SIZE: usize = 16384; // 16KB

pub fn allocate_kernel_stack() -> VirtAddr {
    // Allocate stack on the heap
    let stack_vec = vec![0u8; KERNEL_STACK_SIZE];
    let stack_box = stack_vec.into_boxed_slice();

    // Leak the box to get a static lifetime (we don't want it freed)
    let stack_bottom = Box::leak(stack_box).as_ptr();

    // Return the TOP of the stack
    let stack_top = unsafe {
        VirtAddr::from_ptr(stack_bottom.add(KERNEL_STACK_SIZE))
    };

    stack_top
}

pub fn allocate_user_stack() -> VirtAddr {
    // Allocate user stack on the heap
    let stack_vec = vec![0u8; USER_STACK_SIZE];
    let stack_box = stack_vec.into_boxed_slice();

    // Leak the box to get a static lifetime
    let stack_bottom = Box::leak(stack_box).as_ptr();

    // Return the TOP of the stack
    let stack_top = unsafe {
        VirtAddr::from_ptr(stack_bottom.add(USER_STACK_SIZE))
    };

    stack_top
}

/// Create a new page table for a process by copying the kernel's page table
/// This provides memory isolation while still allowing access to kernel code
pub unsafe fn create_process_page_table(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<PhysFrame, &'static str> {
    // Allocate a new frame for the level 4 page table
    let frame = frame_allocator
        .allocate_frame()
        .ok_or("Failed to allocate frame for page table")?;

    // Get the kernel's level 4 table
    let kernel_table = mapper.level_4_table();

    // Get a mutable reference to the new page table
    let phys_offset = VirtAddr::new(0); // This should be the physical memory offset
    let new_table_addr = phys_offset + frame.start_address().as_u64();
    let new_table: &mut PageTable = &mut *(new_table_addr.as_mut_ptr());

    // Copy kernel mappings (upper half) to the new table
    // This allows the process to access kernel code while having its own user space
    for i in 256..512 {
        new_table[i] = kernel_table[i].clone();
    }

    // Clear user space mappings (lower half)
    for i in 0..256 {
        new_table[i].set_unused();
    }

    Ok(frame)
}

/// Free the memory associated with a process
pub unsafe fn free_process_memory(
    kernel_stack: VirtAddr,
    user_stack: VirtAddr,
) {
    // In a more complete implementation, we would:
    // 1. Free the page table and all user-space pages
    // 2. Free the kernel stack
    // 3. Free the user stack
    //
    // For now, since we're using Box::leak(), the memory will remain
    // allocated until the kernel shuts down. This is acceptable for
    // a simple kernel but should be improved in production.

    // NOTE: To properly free leaked boxes, we would need to:
    // 1. Keep track of the original allocation sizes
    // 2. Use Box::from_raw() to recreate the box
    // 3. Let it drop naturally
    // This is left as a future enhancement.
}