extern crate termcolor;

use std::io::Write;

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor, BufferWriter, BufferedStandardStream};

fn main() {
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);
    stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
    writeln!(&mut stdout, "green text!").unwrap();

    let mut bss = BufferedStandardStream::stdout(ColorChoice::Auto);
    bss.set_color(ColorSpec::new().set_fg(Some(Color::Green))).unwrap();
    writeln!(&mut bss, "buffered text 1!").unwrap();

    let buffer_writer = BufferWriter::stdout(ColorChoice::Auto);
    let mut buffer = buffer_writer.buffer();
    writeln!(&mut buffer, "buffered text 2!").unwrap();
    buffer_writer.print(&buffer).unwrap();
}
