//! Lang item required to make the normal `main` work in applications
//!
//! This is how the `start` lang item works:
//! When `rustc` compiles a binary crate, it creates a `main` function that looks
//! like this:
//!
//! ```
//! #[export_name = "main"]
//! pub extern "C" fn rustc_main(argc: isize, argv: *const *const u8) -> isize {
//!     start(main, argc, argv)
//! }
//! ```
//!
//! Where `start` is this function and `main` is the binary crate's `main`
//! function.
//!
//! The final piece is that the entry point of our program, _start, has to call
//! `rustc_main`. That's covered by the `_start` function in the root of this
//! crate.

#[cfg(feature = "panic_console")]
use crate::console::Console;
use crate::led;
use crate::timer;
use crate::timer::Duration;
use core::alloc::Layout;
#[cfg(feature = "panic_console")]
use core::fmt::Write;
use core::panic::PanicInfo;

#[lang = "start"]
extern "C" fn start<T>(main: fn() -> T, _argc: isize, _argv: *const *const u8) -> i32
where
    T: Termination,
{
    main().report()
}

pub trait Termination {
    fn report(self) -> i32;
}

impl Termination for () {
    fn report(self) -> i32 {
        0
    }
}

fn flash_all_leds() -> ! {
    // Flash all LEDs (if available).
    loop {
        for led in led::all() {
            led.on();
        }
        timer::sleep(Duration::from_ms(100));
        for led in led::all() {
            led.off();
        }
        timer::sleep(Duration::from_ms(100));
    }
}

#[cfg(feature = "panic_console")]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    // Signal a panic using the LowLevelDebug capsule (if available).
    super::debug::low_level_status_code(1);

    let mut console = Console::new();
    writeln!(console, "{}", info).ok();
    // Force the kernel to report the panic cause, by reading an invalid address.
    // The memory protection unit should be setup by the Tock kernel to prevent apps from accessing
    // address zero.
    unsafe {
        core::ptr::read_volatile(0 as *const usize);
    }
    // We still flash the LEDs in case for some reason the previous line didn't cause a crash in
    // the kernel.
    flash_all_leds();
}

#[cfg(not(feature = "panic_console"))]
#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    // Signal a panic using the LowLevelDebug capsule (if available).
    super::debug::low_level_status_code(1);

    flash_all_leds();
}

#[alloc_error_handler]
fn cycle_leds(_: Layout) -> ! {
    loop {
        for led in led::all() {
            led.on();
            timer::sleep(Duration::from_ms(100));
            led.off();
        }
    }
}
