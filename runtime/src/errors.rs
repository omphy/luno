use std::fmt;

#[derive(Debug)]
pub enum RuntimeError {
    NotFound(String),
    TypeError(String),
    InternalError(String),
    ConversionError(String),
    ThreadError(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            RuntimeError::TypeError(msg) => write!(f, "Type Error: {}", msg),
            RuntimeError::InternalError(msg) => write!(f, "Internal Error: {}", msg),
            RuntimeError::ConversionError(msg) => write!(f, "Conversion Error: {}", msg),
            RuntimeError::ThreadError(msg) => write!(f, "Thread Error: {}", msg),
        }
    }
}

#[derive(Debug)]
pub enum CompileError {
    NotFound(String),
    TypeError(String),
    InternalError(String),
    ConversionError(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            CompileError::TypeError(msg) => write!(f, "Type Error: {}", msg),
            CompileError::InternalError(msg) => write!(f, "Internal Error: {}", msg),
            CompileError::ConversionError(msg) => write!(f, "Conversion Error: {}", msg),
        }
    }
}