//! Provides facilities for generating error enums representing a subset of the
//! kernel's error codes (defined at
//! https://github.com/tock/tock/blob/master/kernel/src/returncode.rs). To make
//! conversions more efficient, they have the same numeric values as the
//! ReturnCode values. They have fewer possibilites to prevent unnecessary match
//! branches.

/// Generates an enum with the specified subset of ReturnCode's values. For
/// example:
/// ```
/// returncode_subset![ enum Error {
///     SUCCESS,
///     EOFF,
/// } ];
/// ```
/// expands to:
/// ```
/// enum Error {
///     SUCCESS = 0,
///     EOFF = -4,
/// };
/// ```
// TODO: Do we want to auto-implement TryFrom for the type? It's unclear whether
// most uses of these structs would want to use custom code for each case or
// whether there would be a significant number of drivers that would translate
// directly. This may also change if we add a 1-word ReturnCode type that packs
// a Result<positive isize, Error> into a single struct.
// TODO: Do we want to have conversions between different ReturnCode subsets? If
// so, how? Fallable at runtime? Enforced at compile-time (one trait per
// ReturnCode variant?).
// TODO: What is the backwards-compatibility story here? Can syscall APIs add
// new errors codes as they wish? If not, then yay. If yes, do we want to do
// something like #[non_exhaustive]? Do we want to force the userspace drivers
// to coerce errors to the errors they've stabilized?
#[macro_export]
macro_rules! returncode_subset {
    [$p:vis enum $name:ident { $($v:ident),* }] => {
        $p enum $name { $($v = $crate::returncode_value![$v]),* }
    };
    [enum $name:ident { $($v:ident),* }] => {
        enum $name { $($v = $crate::returncode_value![$v]),* }
    };
}

#[macro_export]
macro_rules! returncode_value {
    [SUCCESS] => (0);
    [FAIL] => (-1);
    [EBUSY] => (-2);
    [EALREADY] => (-3);
    [EOFF] => (-4);
    [ERESERVE] => (-5);
    [EINVAL] => (-6);
    [ESIZE] => (-7);
    [ECANCEL] => (-8);
    [ENOMEM] => (-9);
    [ENOSUPPORT] => (-10);
    [ENODEVICE] => (-11);
    [EUNINSTALLED] => (-12);
    [ENOACK] => (-13);
}
