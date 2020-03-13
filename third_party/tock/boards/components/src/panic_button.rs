//! Component to cause a button press to trigger a kernel panic.
//!
//! This can be useful especially when developping or debugging console
//! capsules.
//!
//! Usage
//! -----
//!
//! ```rust
//! components::panic_button::PanicButtonComponent::new(
//!     &sam4l::gpio::PC[24],
//!     kernel::hil::gpio::ActivationMode::ActiveLow,
//!     kernel::hil::gpio::FloatingState::PullUp
//! ).finalize(());
//! ```

use capsules::panic_button::PanicButton;
use kernel::component::Component;
use kernel::hil::gpio;
use kernel::static_init;

pub struct PanicButtonComponent<'a> {
    pin: &'a dyn gpio::InterruptPin,
    mode: gpio::ActivationMode,
    floating_state: gpio::FloatingState,
}

impl<'a> PanicButtonComponent<'a> {
    pub fn new(
        pin: &'a dyn gpio::InterruptPin,
        mode: gpio::ActivationMode,
        floating_state: gpio::FloatingState,
    ) -> Self {
        PanicButtonComponent {
            pin,
            mode,
            floating_state,
        }
    }
}

impl Component for PanicButtonComponent<'static> {
    type StaticInput = ();
    type Output = ();

    unsafe fn finalize(self, _: Self::StaticInput) -> Self::Output {
        let panic_button = static_init!(
            PanicButton,
            PanicButton::new(self.pin, self.mode, self.floating_state)
        );
        self.pin.set_client(panic_button);
    }
}
