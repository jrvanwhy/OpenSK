use crate::lw::time::{AlarmClock, AlarmFired};

pub trait Node: Copy {
    type Clock: AlarmClock;
    type List: crate::lw::async_util::List<AlarmFired>;

    fn get_clock(self) -> &'static Self::Clock;
    fn get_list(self) -> &'static Self::List;
}

pub struct VirtTime<N: Node> {
    node: N,
}

impl<N: Node> VirtTime<N> {
    pub const fn new(node: N) -> VirtTime<N> {
        VirtTime { node }
    }
}
