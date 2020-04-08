pub mod async_util;
pub mod button;
pub mod deferred;
pub mod dyncall;
pub mod led;
pub mod returncode;
pub mod rng;
pub mod time;

// TODO: Figure out a way to initialize drivers that want or need runtime
// initialization (e.g. calling subscribe() to set up static callbacks). We may
// be able to use some sort of "init token" system (implemented kinda like
// capabilities) to show that a driver is initialized -- but note that the
// system cannot be trusted for safety unless we have a way to show the
// particular instance was initialized. Even then, for subscriptions different
// instances can overwrite each other, although that shouldn't result in safety
// issues. That said, subscriptions may show up in the syscall traits, so
// subscription initialization and clashes may not be a problem in the first
// place.
