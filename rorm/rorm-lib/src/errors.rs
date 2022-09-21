use crate::FFIString;

/**
Representation of all error codes.
 */
#[repr(C)]
pub enum Error<'a> {
    /// Everything's fine, nothing to worry about.
    NoError,
    /// Runtime was destroyed and can no longer be accessed.
    MissingRuntimeError,
    /// An error occurred while getting or accessing the runtime.
    RuntimeError(FFIString<'a>),
    /// An error occurred while trying to convert a FFIString into a &str
    InvalidStringError,
}
