use std::{error::Error, fmt, io, str::Utf8Error};
use parquet::errors::ParquetError;
use wasm_bindgen::{JsCast, JsValue};

#[derive(Debug)]
pub struct JsValueError(pub JsValue);

#[derive(Debug)]
pub enum DvisorError {
    Parquet(ParquetError),
    Json(serde_json::Error),
    Io(io::Error),
    UTF8(Utf8Error),
    Msg(String),
    Js(JsValueError)
}

pub type DvisorResult<T> = std::result::Result<T, DvisorError>;


impl fmt::Display for JsValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(s) = self.0.as_string() {
            write!(f, "{}", s)
        } else if let Some(e) = self.0.dyn_ref::<js_sys::Error>() {
            // A JS Error (e.g. thrown by js_fetch_range on an HTTP failure):
            // its message alone, not the Debug dump, which embeds the whole
            // stack trace in what ends up as a user-facing message.
            write!(f, "{}", String::from(e.message()))
        } else {
            write!(f, "{:?}", self.0)
        }
    }
}
impl Error for JsValueError {}

impl fmt::Display for DvisorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DvisorError::Parquet(e) => write!(f, "{e}"),
            DvisorError::Json(e) => write!(f, "JSON error: {e}"),
            DvisorError::Io(e) => write!(f, "I/O error: {e}"),
            DvisorError::UTF8(e) => write!(f, "UTF8 error: {e}"),
            DvisorError::Js(e) => write!(f, "JsValue error: {e}"),
            DvisorError::Msg(msg) => write!(f, "Dvisor Error: {msg}"),
        }
    }
}

impl Error for DvisorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DvisorError::Parquet(e) => Some(e),
            DvisorError::Json(e) => Some(e),
            DvisorError::Io(e) => Some(e),
            DvisorError::UTF8(e) => Some(e),
            DvisorError::Js(e) => Some(e),
            DvisorError::Msg(_) => None,
        }
    }
}

impl From<io::Error> for DvisorError {
    fn from(e: io::Error) -> Self {
        DvisorError::Io(e)
    }
}

impl From<ParquetError> for DvisorError {
    fn from(e: ParquetError) -> Self {
        DvisorError::Parquet(e)
    }
}

impl From<String> for DvisorError {
    fn from(value: String) -> Self {
        DvisorError::Msg(value)
    }
}

impl From<&str> for DvisorError {
    fn from(value: &str) -> Self {
        DvisorError::Msg(value.to_string())
    }
}

impl From<serde_json::Error> for DvisorError {
    fn from(e: serde_json::Error) -> Self {
        DvisorError::Json(e)
    }
}

impl From<Utf8Error> for DvisorError {
    fn from(e: Utf8Error) -> Self {
        DvisorError::UTF8(e)
    }
}


impl From<DvisorError> for ParquetError {
    fn from(value: DvisorError) -> Self {
        ParquetError::EOF(value.to_string())
    }
}

impl From<DvisorError> for io::Error {
    fn from(value: DvisorError) -> Self {
        match value {
            DvisorError::Parquet(parquet_error) => parquet_error.into(),
            DvisorError::Json(error) => error.into(),
            DvisorError::Io(error) => error,
            DvisorError::UTF8(error) => io::Error::new(io::ErrorKind::Other, error.to_string()),
            DvisorError::Js(error) => io::Error::new(io::ErrorKind::Other, error.to_string()),
            DvisorError::Msg(msg) => io::Error::new(io::ErrorKind::Other, msg),
        }
    }
}

#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {{
        Err(crate::error::DvisorError::Msg(format!($($arg)*)))
    }};
}

#[macro_export]
macro_rules! throw {
    ($($arg:tt)*) => {
        return $crate::err!($($arg)*)
    }
}

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !$cond {
            $crate::throw!($($arg)*);
        }
    };
}

impl From<DvisorError> for wasm_bindgen::JsValue {
    fn from(value: DvisorError) -> Self {
        wasm_bindgen::JsValue::from(format!("{}",value))
    }
}

impl From<wasm_bindgen::JsValue> for DvisorError {
    fn from(value: wasm_bindgen::JsValue) -> Self {
        DvisorError::Js(JsValueError(value))
    }
}

#[macro_export]
macro_rules! console {
    ($($arg:tt)*) => {{
        web_sys::console::log_1(&format!($($arg)*).into())
    }};
}

