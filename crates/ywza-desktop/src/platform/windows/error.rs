use crate::Error;

impl From<::windows::core::Error> for Error {
    fn from(e: ::windows::core::Error) -> Self {
        Error::Os(e.to_string())
    }
}
