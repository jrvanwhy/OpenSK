//! Interface to the RNG capsule. Fills a provided buffer asynchronously with
//! random bytes, then passes it back to the caller. Does not provide
//! virtualization.
//!
//! RNG has a generic argument (ClientLink) which allows RNG to perform
//! callbacks on its client.

const BUFFER_NUM: usize = 0;
const DRIVER_NUM: usize = 0x40001;
const GET_BYTES: usize = 1;
const GET_BYTES_DONE: usize = 0;

/// Trait that must be implemented by RNG's clients.
pub trait Client {
    fn fetch_done(buffer: &'static mut [u8]);
}

/// A ClientLink gives RNG the ability to pass buffers back to the client. It
/// behaves like a pointer to the RNG's client, but may be implemented as a
/// zero-sized type.
pub trait ClientLink {
    type Client: Client;

    fn get(&self) -> &Self::Client;
}

/// The RNG driver itself.
pub struct Rng<CL: ClientLink> {
    client_link: CL,

    // The buffer corresponding to an ongoing fetch. Null if there is no ongoing
    // fetch. Stored as a raw pointer and length to avoid the undefined behavior
    // that would result from holding a &[u8] pointing to data the kernel is
    // mutating.
    buffer_data: *mut u8,
    buffer_len: usize,
}

impl<CL: ClientLink> Rng<CL> {
    pub fn new(client_link: CL) -> Rng<CL> {
        Rng { client_link, buffer_data: core::ptr::null_mut(), buffer_len: 0 }
    }

    pub fn fetch(&'static self, buffer: &'static mut [u8]) -> Result<(), (Error, &'static mut [u8])> {
        if !self.buffer_data.is_null() { return Err((Error::EBUSY, buffer)); }
        match crate::syscalls::allow_ptr(DRIVER_NUM, BUFFER_NUM, buffer.as_mut_ptr(), buffer.len()) {
            0 => {},  // Success
            -10 => return Err((Error::ENOSUPPORT, buffer)),
            _ => return Err((Error::FAIL, buffer)),
        }
        match crate::syscalls::subscribe_ptr(DRIVER_NUM, GET_BYTES_DONE,
                                             callback::<CL> as *const _,
                                             self as *const Self as usize) {
            0 => {},  // Success
            _ => {
                // Open question: do we need to check this for failure, and if
                // it fails, what do we do? panic? Poison the RNG?
                crate::syscalls::allow_ptr(DRIVER_NUM, BUFFER_NUM, core::ptr::null_mut(), 0);
                return Err((Error::FAIL, buffer));
            },
        }
        //match crate::syscalls::command(DRIVER_NUM, GET_BYTES, buffer.len())
    }
}

/// Error type for the RNG driver.
pub enum Error {
    FAIL = -1,         // Internal failure
    EBUSY = -2,        // A fetch is ongoing.
    ENOSUPPORT = -10,  // The kernel does not have the RNG capsule.
}

// We don't use crate::syscalls::subscribe as that requires a unique reference
// and we need subscribe to work with a shared reference. This is the callback
// we use instead.
extern "C" fn callback<CL: ClientLink>(_: usize, _: usize, _: usize, rng: usize) {
	
}
