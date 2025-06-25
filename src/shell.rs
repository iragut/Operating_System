use core::{char, fmt::Write};
use alloc::{string::String, vec::Vec};
use pc_keyboard::KeyCode;
use spin::Mutex;
use lazy_static::lazy_static;

use crate::println;

lazy_static! {
    pub static ref SHELL: Mutex<Shell> = Mutex::new(Shell::new());
}

pub struct Shell {
    index_buf: i32,
    buffer: String,
    history: Vec<String>,
    commands: Vec<(&'static str, fn(&str, &mut Shell))>,
}

impl Shell {
    pub fn new() -> Self {
       let mut shell = Self {
            index_buf: 0,
            buffer: String::new(),
            history: Vec::new(),
            commands: Vec::new(),
        };
        shell.add_command("help", Shell::help_command);
        shell
    }

    fn shift_all_charcters_left(&mut self) {
        let mut writer = crate::vga_buffer::WRITER.lock();
        for i in 0..self.index_buf {
            let c = self.buffer.chars().nth(i as usize).unwrap_or(' ');
            writer.move_cursor_left();
            let _ = writer.write_char(c);
        }
    }

    pub fn handle_char(&mut self, c: char) {
        match c {
            '\x08' => {
                if self.buffer.is_empty() {
                    return;
                } else {
                    let mut writer = crate::vga_buffer::WRITER.lock();

                    // if self.index_buf > writer.get_column_position() {
                    //     self.buffer.remove(self.index_buf as usize);

                    // }
                    
                    self.index_buf -= 1;
                    self.buffer.pop();

                    writer.move_cursor_left();
                    let _ = writer.write_char(' ');
                    writer.move_cursor_left();
                    
                }
            }

            '\n' => {
                if self.buffer.is_empty() {
                    return;
                }
                self.history.push(self.buffer.clone());


                if let Some((_, func)) = self.commands.iter().find(|(name, _)| name == &self.buffer) {
                    let str = self.buffer.clone();
                    func(&str, self);
                } else {
                    println!("Unknown command: {}", self.buffer);
                }

                
                self.index_buf = 0;
                self.buffer.clear();
            }

            _ => {
                self.buffer.push(c);
                self.index_buf += 1;
                let mut writer = crate::vga_buffer::WRITER.lock();
                let _ = writer.write_char(c);
            }
        }

    }

    pub fn handle_special(&mut self, key: KeyCode){
        match key {
            KeyCode::ArrowLeft => {
                let mut writer = crate::vga_buffer::WRITER.lock();
                writer.move_cursor_left();
            }
            KeyCode::ArrowRight => {
                let mut writer = crate::vga_buffer::WRITER.lock();
                writer.move_cursor_right();
            }
            _ => {}
            
        }

    }

    pub fn add_command(&mut self, name: &'static str, func: fn(&str, &mut Shell)) {
        self.commands.push((name, func));
    }

    fn help_command(_args: &str, shell: &mut Shell) {
        println!("\nAvailable commands:");
        for (name, _) in &shell.commands {
            println!("- {}", name);
        }
    }
    
}