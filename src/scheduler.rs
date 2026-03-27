use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};

use core::cell::UnsafeCell;

use x86_64::structures::paging::PageTableFlags;
use x86_64::registers::control::Cr3;
use x86_64::VirtAddr;

use crate::asm_switch::CpuState;
use crate::memory::allocate_kernel_stack;
use crate::process::{ProcessBlock, ProcessMemory, ProcessState};


pub struct SchedulerCell(UnsafeCell<ProcessManager>);

unsafe impl Sync for SchedulerCell {}
unsafe impl Send for SchedulerCell {}

impl SchedulerCell {
    const fn new(pm: ProcessManager) -> Self {
        SchedulerCell(UnsafeCell::new(pm))
    }

    // Caller must ensure interrupts are disabled.
    pub unsafe fn get(&self) -> &mut ProcessManager {
        &mut *self.0.get()
    }
}

pub static SCHEDULER: SchedulerCell = SchedulerCell::new(ProcessManager::new());

pub struct ProcessManager {
    pub processes: BTreeMap<u32, Box<ProcessBlock>>,
    pub ready_queue: VecDeque<u32>,
    pub current_pid: Option<u32>,
    pub next_pid: u32,
}

impl ProcessManager {
    pub const fn new() -> Self {
        ProcessManager {
            processes: BTreeMap::new(),
            ready_queue: VecDeque::new(),
            current_pid: None,
            next_pid: 1,
        }
    }

    pub fn get_current_pid(&self) -> Option<u32> {
        self.current_pid
    }

    pub fn schedule(&mut self) -> Option<u32> {
        // Move current to ready queue if still running
        if let Some(current) = self.current_pid {
            if let Some(proc) = self.processes.get_mut(&current) {
                if matches!(proc.state, ProcessState::Running) {
                    proc.state = ProcessState::Ready;
                    self.ready_queue.push_back(current);
                }
            }
        }

        // Get next from ready queue
        if let Some(next_pid) = self.ready_queue.pop_front() {
            if let Some(proc) = self.processes.get_mut(&next_pid) {
                proc.state = ProcessState::Running;
                self.current_pid = Some(next_pid);
                return Some(next_pid);
            }
        }

        self.current_pid
    }

   pub fn create_process(&mut self, program: &[u8]) -> u32 {
        const USER_CODE_ADDR: u64 = 0x400000;
        const USER_STACK_TOP: u64 = 0x800000;
        const USER_STACK_BOTTOM: u64 = USER_STACK_TOP - 4096;

        let user_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;
        let user_stack_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

        let pid = self.next_pid;
        self.next_pid += 1;

        let kernel_stack = allocate_kernel_stack();
        let stack_top = kernel_stack.as_u64();
        let state_ptr = (stack_top - core::mem::size_of::<CpuState>() as u64) as *mut CpuState;

        unsafe {
            *state_ptr = CpuState {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: 0,
                r11: 0,
                r10: 0,
                r9: 0,
                r8: 0,
                rbp: 0,
                rdi: 0,
                rsi: 0,
                rdx: 0,
                rcx: 0,
                rbx: 0,
                rax: 0,
                rip: USER_CODE_ADDR,
                cs: crate::gdt::user_code_selector().0 as u64,
                rflags: 0x202,
                rsp: USER_STACK_TOP,
                ss: crate::gdt::user_data_selector().0 as u64,
            };
        }

        let phys_mem_offset = unsafe {
            VirtAddr::new(crate::memory::PHYS_MEM_OFFSET)
        };
        let frame_alloc = unsafe { crate::memory::FRAME_ALLOCATOR.get() };
        let page_table_frame = crate::memory::create_process_page_table(frame_alloc, phys_mem_offset);

        // Map code pages
        let num_pages = (program.len() + 4095) / 4096;
        let mut offset = 0;

        for i in 0..num_pages {
            let page_addr = USER_CODE_ADDR + (i as u64 * 4096);
            let code_frame = crate::memory::map_user_page(
                page_table_frame, phys_mem_offset,
                frame_alloc, VirtAddr::new(page_addr), user_flags,
            );

            let dst = (phys_mem_offset + code_frame.start_address().as_u64()).as_mut_ptr::<u8>();
            let remaining = program.len() - offset;
            let to_copy = if remaining > 4096 { 4096 } else { remaining };

            unsafe {
                core::ptr::copy_nonoverlapping(program[offset..].as_ptr(), dst, to_copy);
                // Zero the rest of the page if partial
                if to_copy < 4096 {
                    core::ptr::write_bytes(dst.add(to_copy), 0, 4096 - to_copy);
                }
            }

            offset += 4096;
        }

        // Map stack page
        crate::memory::map_user_page(
            page_table_frame, phys_mem_offset,
            frame_alloc, VirtAddr::new(USER_STACK_BOTTOM), user_stack_flags,
        );

        let process = Box::new(ProcessBlock {
            pid,
            state: ProcessState::Ready,
            priority: 1,
            parent_pid: 0,
            saved_state: state_ptr,
            memory: ProcessMemory::new(
                page_table_frame.start_address(),
                VirtAddr::new(USER_CODE_ADDR),
                VirtAddr::new(0),
                VirtAddr::new(crate::allocator::HEAP_START as u64),
                kernel_stack,
            ),
            kernel_stack,
            time: 0,
        });

        self.processes.insert(pid, process);
        self.ready_queue.push_back(pid);
        pid
    }

    pub fn init_kernel_process(&mut self) {
        let process_zero = Box::new(ProcessBlock {
            pid: 0,
            state: ProcessState::Running,
            priority: 1,
            parent_pid: 0,
            saved_state: core::ptr::null_mut(),
            memory: ProcessMemory::new(
                Cr3::read().0.start_address(),
                VirtAddr::new(0x200000),
                VirtAddr::new(0x300000),
                VirtAddr::new(crate::allocator::HEAP_START as u64),
                VirtAddr::new(0),
            ),
            kernel_stack: VirtAddr::new(0),
            time: 0,
        });

        self.processes.insert(0, process_zero);
        self.current_pid = Some(0);
    }

    pub fn terminate_process(&mut self, pid: u32) {
        let process = self.processes.get_mut(&pid);

        if process.is_none() {
            return;
        }

        let process = process.unwrap();

        if self.current_pid == Some(pid) {
            self.current_pid = self.ready_queue.front().copied();
        }

        process.state = ProcessState::Terminated;
        self.ready_queue.retain(|&p| p != pid);

        if self.current_pid == Some(pid) {
            self.current_pid = None;
        }
    }

    // Function to use for testing
    pub fn reset(&mut self) {
        self.processes.clear();
        self.ready_queue.clear();
        self.current_pid = None;
        self.next_pid = 1;
    }
}
