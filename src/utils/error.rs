use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("FFmpeg error: {message}")]
    Ffmpeg { message: String },

    #[error("Video analysis error: {message}")]
    Analysis { message: String },

    #[error("Profile error: {message}")]
    Profile { message: String },

    #[error("Encoding error: {message}")]
    Encoding { message: String },

    #[error("Progress tracking error: {message}")]
    Progress { message: String },

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Parse error: {message}")]
    Parse { message: String },

    #[error("Validation error: {message}")]
    Validation { message: String },


    #[error("Dolby Vision error: {0}")]
    DolbyVision(String),
}

impl Error {
    pub fn ffmpeg<T: Into<String>>(message: T) -> Self {
        Self::Ffmpeg {
            message: message.into(),
        }
    }

    pub fn analysis<T: Into<String>>(message: T) -> Self {
        Self::Analysis {
            message: message.into(),
        }
    }

    pub fn profile<T: Into<String>>(message: T) -> Self {
        Self::Profile {
            message: message.into(),
        }
    }

    pub fn encoding<T: Into<String>>(message: T) -> Self {
        Self::Encoding {
            message: message.into(),
        }
    }

    pub fn progress<T: Into<String>>(message: T) -> Self {
        Self::Progress {
            message: message.into(),
        }
    }

    pub fn parse<T: Into<String>>(message: T) -> Self {
        Self::Parse {
            message: message.into(),
        }
    }

    pub fn validation<T: Into<String>>(message: T) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }


    pub fn dolby_vision<T: Into<String>>(message: T) -> Self {
        Self::DolbyVision(message.into())
    }
}
