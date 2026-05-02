use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("command not found: '{0}'")]
    CommandNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "script requires Forge Shell >= {required}, but this is {current}\nUpdate with: forge self-update"
    )]
    MinVersionNotMet { required: String, current: String },

    #[error("script declares platform = {declared} and cannot run on {current}")]
    PlatformNotSupported { declared: String, current: String },

    #[error("required environment variable(s) not set: {vars}")]
    RequiredEnvMissing { vars: String },

    #[error("script exceeded timeout of {timeout}")]
    Timeout { timeout: String },

    #[error("variable '{name}' is not defined")]
    UndefinedVariable { name: String },

    #[error("division by zero")]
    DivisionByZero,

    #[error("integer overflow")]
    IntegerOverflow,

    #[error("type error: cannot apply {op} to {left} and {right}")]
    TypeError {
        op: String,
        left: String,
        right: String,
    },

    #[error("env file not found: '{path}'")]
    EnvFileNotFound { path: String },

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}
