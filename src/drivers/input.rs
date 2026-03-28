use core::cell::UnsafeCell;

pub struct InputCell(UnsafeCell<Input>);

unsafe impl Sync for InputCell {}
unsafe impl Send for InputCell {}

impl InputCell {
    const fn new() -> Self {
        InputCell(UnsafeCell::new(Input {
            buffer: [0; 256],
            head: 0,
            tail: 0,
            waiting_pid: None,
        }))
    }

    pub unsafe fn get(&self) -> &mut Input {
        &mut *self.0.get()
    }
}

pub static INPUT: InputCell = InputCell::new();

pub struct Input {
    buffer: [u8; 256],
    head: usize,
    tail: usize,
    pub waiting_pid: Option<u32>,
}

impl Input {
    pub fn push(&mut self, byte: u8) {
        let next_head = (self.head + 1) % self.buffer.len();
        if next_head != self.tail {
            self.buffer[self.head] = byte;
            self.head = next_head;

            if let Some(pid) = self.waiting_pid.take() {
                let scheduler = unsafe { crate::proc::scheduler::SCHEDULER.get() };
                if let Some(process) = scheduler.processes.get_mut(&pid) {
                    process.set_state(crate::proc::process::ProcessState::Ready);
                    scheduler.ready_queue.push_back(pid);
                }
            }
        }
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            None
        } else {
            let byte = self.buffer[self.tail];
            self.tail = (self.tail + 1) % self.buffer.len();
            Some(byte)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    pub fn is_full(&self) -> bool {
        (self.head + 1) % self.buffer.len() == self.tail
    }

    pub fn len(&self) -> usize {
        if self.head >= self.tail {
            self.head - self.tail
        } else {
            self.buffer.len() - (self.tail - self.head)
        }
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
    }
}