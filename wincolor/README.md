## **This crate has reached its end-of-life and is now deprecated.**

This crate was rolled into the
[`winapi-util`](https://crates.io/crates/winapi-util)
crate since `wincolor` is quite small and didn't otherwise have a good reason
for living life as a distinct crate.

The
[`console`](https://docs.rs/winapi-util/0.1.*/x86_64-pc-windows-msvc/winapi_util/console/index.html)
module of `winapi-util` is a drop-in replacement for `wincolor`.

wincolor
========
A simple Windows specific API for controlling text color in a Windows console.
The purpose of this crate is to expose the full inflexibility of the Windows
console without any platform independent abstraction.

[![](https://img.shields.io/crates/v/wincolor.svg)](https://crates.io/crates/wincolor)

Dual-licensed under MIT or the [UNLICENSE](https://unlicense.org/).

### Documentation

[https://docs.rs/wincolor](https://docs.rs/wincolor)

### Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
wincolor = "0.1"
```

and this to your crate root:

```rust
extern crate wincolor;
```

### Example

This is a simple example that shows how to write text with a foreground color
of cyan and the intense attribute set:

```rust
use wincolor::{Console, Color, Intense};

let mut con = Console::stdout().unwrap();
con.fg(Intense::Yes, Color::Cyan).unwrap();
println!("This text will be intense cyan.");
con.reset().unwrap();
println!("This text will be normal.");
```
