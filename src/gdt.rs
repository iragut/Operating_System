use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::VirtAddr;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

// Static mutable TSS storage
static mut TSS_STORAGE: TaskStateSegment = TaskStateSegment::new();

lazy_static! {
    // Wrap the TSS in a Mutex for safe access
    pub static ref TSS: Mutex<&'static mut TaskStateSegment> = unsafe {
        // Initialize the double fault stack
        TSS_STORAGE.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            
            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        
        Mutex::new(&mut TSS_STORAGE)
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        
        // Get a reference to the TSS for the GDT
        // We need to access the raw TSS, not through the Mutex
        let tss_selector = unsafe {
            gdt.add_entry(Descriptor::tss_segment(&TSS_STORAGE))
        };
        
        (
            gdt,
            Selectors {
                code_selector,
                tss_selector,
            },
        )
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, Segment};
    use x86_64::instructions::tables::load_tss;

    // Force initialization of TSS before GDT
    lazy_static::initialize(&TSS);
    
    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}