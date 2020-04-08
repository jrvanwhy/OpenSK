/// Futures-based interface to wait for a given amount of time. Uses the
/// lightweight timer/alarm driver.

use core::cell::Cell;
use core::task::{Context, Poll};
use crate::lw::async_util::TockStatic;
use crate::lw::time::{AlarmFired, Clock};

pub struct AlarmFuture {
    setpoint: u64,
}

impl AlarmFuture {
    // Sets an alarm for `delay` ticks in the future.
    pub fn new(delay: u64) -> AlarmFuture {
        if !INITIALIZED.get() { CLOCK.init(); INITIALIZED.set(true); }
        AlarmFuture { setpoint: delay + CLOCK.get_time() }
    }
}

impl core::future::Future for AlarmFuture {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, _cx: &mut Context) -> Poll<()> {
        let cur_alarm = CLOCK.get_alarm();
        if cur_alarm > self.setpoint {
            if CLOCK.set_alarm(self.setpoint).is_ok() {
                return Poll::Pending;
            }
            // I'm not sure whether ignoring this error is a bug. This logic
            // would probably change if we store a Waker.
            let _ = CLOCK.set_alarm(cur_alarm);
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

static INITIALIZED: TockStatic<Cell<bool>> = TockStatic::new(Cell::new(false));
static CLOCK: TockStatic<Clock<FutureForwarder>> = TockStatic::new(Clock::new(FutureForwarder));

#[derive(Clone, Copy)]
struct FutureForwarder;

impl crate::lw::async_util::Forwarder<AlarmFired> for FutureForwarder {
    fn invoke_callback(self, _: AlarmFired) {
        // No-op. The setpoint has already been reset, and the futures that
        // expired will poll CLOCK to determine that they have expired.
        // TODO: This should store a Waker and poll it instead of assuming
        // futures will be polled.
    }
}
