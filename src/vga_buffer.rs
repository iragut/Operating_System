use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;
use x86_64::instructions::port::Port;

lazy_static! {
    /// A global `Writer` instance that can be used for printing to the VGA text buffer.
    ///
    /// Used by the `print!` and `println!` macros.
    pub static ref WRITER: Mutex<Writer> = {
        let mut writer = Writer {
            column_position: 0,
            color_code: ColorCode::new(Color::LightCyan, Color::Black),
            buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
            cursor_x: 0,
            cursor_y: (BUFFER_HEIGHT - 1) as u16,
        };
        
        // Initialize cursor when WRITER is created
        writer.enable_cursor(14, 15);  // Normal cursor
        writer.update_cursor(writer.cursor_x, writer.cursor_y);
        
        Mutex::new(writer)
    };
}

/// The standard color palette in VGA text mode.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// A combination of a foreground and a background color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    /// Create a new `ColorCode` with the given foreground and background colors.
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

/// A screen character in the VGA text buffer, consisting of an ASCII character and a `ColorCode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

/// The height of the text buffer (normally 25 lines).
const BUFFER_HEIGHT: usize = 25;
/// The width of the text buffer (normally 80 columns).
const BUFFER_WIDTH: usize = 80;

/// A structure representing the VGA text buffer.
#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

/// A writer type that allows writing ASCII bytes and strings to an underlying `Buffer`.
///
/// Wraps lines at `BUFFER_WIDTH`. Supports newline characters and implements the
/// `core::fmt::Write` trait.
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
    cursor_x: u16,
    cursor_y: u16,
}

impl Writer {
    pub fn set_column_position(&mut self, position: usize) {
        if position < BUFFER_WIDTH {
            self.column_position = position;
        } else {
            self.column_position = BUFFER_WIDTH - 1; // Clamp to max width
        }
    }
    pub fn get_column_position(&self) -> i32 {
        self.column_position as i32
    }
    pub fn get_cursor_x(&self) -> u16 {
        self.cursor_x
    }
    pub fn get_cursor_y(&self) -> u16 {
        self.cursor_y
    }
    /// Writes an ASCII byte to the buffer.
    ///
    /// Wraps lines at `BUFFER_WIDTH`. Supports the `\n` newline character.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
        self.update_cursor(self.column_position as u16, self.cursor_y);
    }

    /// Writes the given ASCII string to the buffer.
    ///
    /// Wraps lines at `BUFFER_WIDTH`. Supports the `\n` newline character. Does **not**
    /// support strings with non-ASCII characters, since they can't be printed in the VGA text
    /// mode.
    fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // not part of printable ASCII range
                _ => self.write_byte(0xfe),
            }
        }
    }

    /// Shifts all lines one line up and clears the last row.
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
        self.update_cursor(0, (BUFFER_HEIGHT - 1) as u16);
    }

    /// Clears a row by overwriting it with blank characters.
    pub fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }

    pub fn enable_cursor(&mut self, cursor_start: u8, cursor_end: u8) {
        unsafe {
            let mut cmd_port: Port<u8> = Port::new(0x3D4);
            let mut data_port: Port<u8> = Port::new(0x3D5);
            
            // Set cursor start - split read and write
            cmd_port.write(0x0A);
            let current_start = data_port.read();
            data_port.write((current_start & 0xC0) | cursor_start);
            
            // Set cursor end - split read and write  
            cmd_port.write(0x0B);
            let current_end = data_port.read();
            data_port.write((current_end & 0xE0) | cursor_end);
        }
    }
    
    pub fn disable_cursor(&mut self) {
        unsafe {
            let mut cmd_port: Port<u8> = Port::new(0x3D4);
            let mut data_port: Port<u8> = Port::new(0x3D5);
            
            cmd_port.write(0x0A);
            data_port.write(0x20);
        }
    }
    
    pub fn update_cursor(&mut self, x: u16, y: u16) {
        let pos = y * BUFFER_WIDTH as u16 + x;
        
        unsafe {
            let mut cmd_port: Port<u8> = Port::new(0x3D4);
            let mut data_port: Port<u8> = Port::new(0x3D5);
            
            // Send low byte
            cmd_port.write(0x0F);
            data_port.write((pos & 0xFF) as u8);
            
            // Send high byte
            cmd_port.write(0x0E);
            data_port.write((pos >> 8) as u8);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.column_position > 0 {
            self.column_position -= 1;
            self.update_cursor(self.column_position as u16, self.cursor_y);
        }
    }
    
    pub fn move_cursor_right(&mut self) {
        if self.column_position < BUFFER_WIDTH - 1 {
            self.column_position += 1;
            self.update_cursor(self.column_position as u16, self.cursor_y);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Like the `print!` macro in the standard library, but prints to the VGA text buffer.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

/// Like the `println!` macro in the standard library, but prints to the VGA text buffer.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// Prints the given formatted string to the VGA text buffer
/// through the global `WRITER` instance.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}