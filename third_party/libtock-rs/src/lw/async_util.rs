//! Module containing async building blocks used by this lightweight libtock-rs
//! prototype.

/// A trait implemented by clients of asynchronous components. Has a callback
/// that receives a value of type T.
pub trait Client<T> {
    fn callback(&self, response: T);
}

/// A trait for "forwarders", which are type system shims that route callbacks
/// to the appropriate client. Asynchronous components are generally generic
/// over a forwarder; the forwarder provides them a way to route a callback to
/// the client that does not require the asynchronous component to store a
/// pointer to the client.
///
/// The forwarders are Copy and take `self` (rather than `&self`) so that if
/// they are implemented as a zero-sized type the self argument will have no
/// overhead.
pub trait Forwarder<T>: Copy {
    fn invoke_callback(self, response: T);
}
