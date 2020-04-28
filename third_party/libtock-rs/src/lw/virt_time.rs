use crate::lw::async_util::{Client, DynClient};
use crate::lw::time::{AlarmClock, AlarmFired};

pub trait MuxDeps: Copy {
    type Clock: AlarmClock + 'static;
    fn get_clock(self) -> &'static Self::Clock;
}

pub struct Mux<D: MuxDeps> {
    deps: D,
    head: core::cell::Cell<Option<&'static MuxClient>>,
}

impl<D: MuxDeps> Mux<D> {
    pub const fn new(deps: D) -> Mux<D> {
        Mux { deps, head: core::cell::Cell::new(None) }
    }
}

impl<D: MuxDeps> Client<AlarmFired> for Mux<D> {
    fn callback(&self, _response: AlarmFired) {
        let time = self.deps.get_clock().get_time();
        let mut cur_client = self.head.get();
        while let Some(client) = cur_client {
            if let Some(setpoint) = client.setpoint.get() {
                if setpoint <= time {
                    // Note: We zero out the setpoint first so that the setpoint
                    // remains set if the callback re-sets setpoint.
                    client.setpoint.set(None);
                    client.client.callback(AlarmFired);
                }
            }
            cur_client = client.next.get();
        }
        // Note: This is two loops to correctly handle the case that alarms are
        // changed during the execution of the callback.
        let mut next_setpoint = u64::max_value();
        cur_client = self.head.get();
        while let Some(client) = cur_client {
            if let Some(setpoint) = client.setpoint.get() {
                if setpoint < next_setpoint { next_setpoint = setpoint; }
            }
            cur_client = client.next.get();
        }
        if next_setpoint != u64::max_value() {
            let _ = self.deps.get_clock().set_alarm(next_setpoint);
        }
    }
}

pub struct MuxClient {
    client: DynClient<'static, AlarmFired>,
    next: core::cell::Cell<Option<&'static MuxClient>>,
    setpoint: core::cell::Cell<Option<u64>>,
}

impl AlarmClock for MuxClient {
    fn get_time(&self) -> u64 {
        self.mux.
    }

    fn get_alarm(&self) -> u64;
    fn set_alarm(&self, time: u64) -> Result<(), InPast>;
}
