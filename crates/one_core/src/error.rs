use std::fmt;

#[derive(Debug)]
pub enum OneError {
    JsException(JsException),
    CompileError(CompileError),
    OutOfFuel { consumed: u64 },
    OutOfMemory { requested: usize, limit: usize },
    StackOverflow { depth: usize },
    ExecutionTimeout { elapsed_ms: u64 },
    InternalError(String),
    TypeError(String),
    Blocked { operation: String, reason: String },
}

#[derive(Debug, Clone)]
pub struct JsException {
    pub name: String,
    pub message: String,
    pub stack_trace: Vec<StackFrame>,
}

#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function_name: Option<String>,
    pub file_name: Option<String>,
    pub line: u32,
    pub column: u32,
}

pub type OneResult<T> = Result<T, OneError>;

impl OneError {
    pub fn js_exception(name: &str, message: &str) -> Self {
        Self::JsException(JsException {
            name: name.to_string(),
            message: message.to_string(),
            stack_trace: Vec::new(),
        })
    }

    pub fn is_recoverable(&self) -> bool {
        !matches!(self, Self::InternalError(_))
    }

    pub fn is_resumable(&self) -> bool {
        matches!(self, Self::OutOfFuel { .. })
    }

    pub fn js_stack_trace(&self) -> Option<&[StackFrame]> {
        match self {
            Self::JsException(ex) => Some(&ex.stack_trace),
            _ => None,
        }
    }
}

impl fmt::Display for OneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsException(ex) => write!(f, "{}: {}", ex.name, ex.message),
            Self::CompileError(err) => {
                if let Some(file) = &err.file {
                    write!(
                        f,
                        "{file}:{}:{}: {}",
                        err.line, err.column, err.message
                    )
                } else {
                    write!(f, "{}:{}: {}", err.line, err.column, err.message)
                }
            }
            Self::OutOfFuel { consumed } => write!(f, "out of fuel after {consumed} units"),
            Self::OutOfMemory { requested, limit } => {
                write!(
                    f,
                    "out of memory: requested {requested} bytes, limit {limit}"
                )
            }
            Self::StackOverflow { depth } => write!(f, "stack overflow at depth {depth}"),
            Self::ExecutionTimeout { elapsed_ms } => {
                write!(f, "execution timeout after {elapsed_ms}ms")
            }
            Self::InternalError(msg) => write!(f, "internal error: {msg}"),
            Self::TypeError(msg) => write!(f, "type error: {msg}"),
            Self::Blocked { operation, reason } => {
                write!(f, "blocked: {operation} — {reason}")
            }
        }
    }
}

impl std::error::Error for OneError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_exception_is_recoverable() {
        let err = OneError::js_exception("TypeError", "x is not a function");
        assert!(err.is_recoverable());
        assert!(!err.is_resumable());
    }

    #[test]
    fn out_of_fuel_is_resumable() {
        let err = OneError::OutOfFuel { consumed: 1000 };
        assert!(err.is_recoverable());
        assert!(err.is_resumable());
    }

    #[test]
    fn internal_error_is_not_recoverable() {
        let err = OneError::InternalError("gc panic".into());
        assert!(!err.is_recoverable());
        assert!(!err.is_resumable());
    }

    #[test]
    fn compile_error_display() {
        let err = OneError::CompileError(CompileError {
            message: "Unexpected token".into(),
            file: Some("test.js".into()),
            line: 10,
            column: 5,
        });
        let msg = format!("{err}");
        assert!(msg.contains("Unexpected token"));
        assert!(msg.contains("test.js"));
    }

    #[test]
    fn js_exception_stack_trace() {
        let err = OneError::JsException(JsException {
            name: "TypeError".into(),
            message: "x is not a function".into(),
            stack_trace: vec![StackFrame {
                function_name: Some("foo".into()),
                file_name: Some("a.js".into()),
                line: 1,
                column: 10,
            }],
        });
        let trace = err.js_stack_trace().unwrap();
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].function_name.as_deref(), Some("foo"));
    }

    #[test]
    fn error_implements_std_error() {
        let err = OneError::js_exception("Error", "something failed");
        let std_err: &dyn std::error::Error = &err;
        assert!(std_err.to_string().contains("something failed"));
    }
}
