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
    pub buffer: String,
    pub history: Vec<String>,
    pub commands: Vec<(&'static str, fn(&str, &mut Shell))>,
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

    fn delete_char_and_redraw(&mut self) {
        let mut writer = crate::vga_buffer::WRITER.lock();
        
        let index = writer.get_column_position() - 1;
        self.buffer.remove(index as usize);
        self.index_buf -= 1;
        
        let row = writer.get_cursor_y() as usize;
        writer.clear_row(row);
        writer.set_column_position(0);
        
        for ch in self.buffer.chars() {
            let _ = writer.write_char(ch);
        }
        
        writer.move_cursor_left();
        writer.move_cursor_left();
    }

    pub fn handle_char(&mut self, c: char) {
        match c {
            '\x08' => {
                if self.buffer.is_empty() {
                    return;
                } else {
                    let mut writer = crate::vga_buffer::WRITER.lock();

                    if self.index_buf > writer.get_column_position() && writer.get_column_position() > 0 {
                        drop(writer);
                        self.delete_char_and_redraw();
                    } else if writer.get_column_position() > 0 {
                    
                        self.index_buf -= 1;
                        self.buffer.pop();

                        writer.move_cursor_left();
                        let _ = writer.write_char(' ');
                        writer.move_cursor_left();
                    }
                    
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
                    println!("\nUnknown command: {}", self.buffer);
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
            KeyCode::ArrowUp => {
                // TODO: Implement history navigation
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
