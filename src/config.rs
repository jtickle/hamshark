use std::{env, fs, path::{Path, PathBuf}};
use directories::{ProjectDirs, UserDirs};

use serde::{Deserialize, Serialize};

use thiserror::Error;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "JeffTickle";
const APPLICATION: &str = "Hamshark";
const SETTINGSFILE: &str = "hamshark.toml";

const HAMSHARK_SETTINGS_FILE_ENV: &str = "HAMSHARK_SETTINGS_FILE";

// Application configuration. Not user-servicible but environment variables
// can generally override.
#[derive(Debug, Clone)]
pub struct Configuration {
    pub settings_file_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("Unable to resolve the OS-specific Settings Path automatically. You can specify one in the {} environment variable.", HAMSHARK_SETTINGS_FILE_ENV)]
    SettingsPathResolution,
}

pub type ConfigurationResult = Result<Configuration, ConfigurationError>;

// User-defined settings. Try to determine sensible defaults.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Settings {
    pub session_base_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("Serialization error writing Hamshark settings: {0}")]
    SerializationError(#[source] toml::ser::Error),
    #[error("Deserialization error reading Hamshark settings: {0}")]
    DeserializationError(#[source] toml::de::Error),
    #[error("Error reading Hamshark settings file: {0}")]
    FileReadError(#[source] std::io::Error),
    #[error("Error writing Hamshark settings file: {0}")]
    FileWriteError(#[source] std::io::Error),
    #[error("Error determining Hamshark settings file existence: {0}")]
    FileExistenceError(#[source] std::io::Error),
    #[error("Error creating Hamshark settings directory: {0}")]
    DirectoryCreationError(#[source] std::io::Error),
}

pub type OwnedSettingsResult = Result<Settings, SettingsError>;
pub type SettingsResult = Result<(), SettingsError>;

impl Configuration {
    pub fn from_env() -> ConfigurationResult {
        // Who knew figuring out the settings file path was going
        // to take so much damn code
        let settings_file_base = match env::var_os(HAMSHARK_SETTINGS_FILE_ENV) {
            Some(env_config_path) => {
                // Path is set in environment so we are going to go with it
                // and if it's invalid, too bad panic
                PathBuf::from(env_config_path)
            },
            None => {
                // Try auto-determining settings dir from OS paths
                match ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION) {
                    // Able to determine the OS-specific config dir
                    Some(project_dirs) => PathBuf::from(project_dirs.config_dir()),
                    // Unable to determine where settings should be stored
                    None => return Err(ConfigurationError::SettingsPathResolution),
                }
            },
        };

        Ok(Self {
            settings_file_path: settings_file_base.join(SETTINGSFILE),
        })
    }
}

impl Settings {
    pub fn from_file(file: &Path) -> OwnedSettingsResult {
        match fs::exists(file) {
            Ok(true) => {
                match fs::read_to_string(file) {
                    Ok(serialized) => match toml::from_str(serialized.as_str()) {
                        Ok(settings) => Ok(settings),
                        Err(error) => Err(SettingsError::DeserializationError(error)),
                    },
                    Err(error) => Err(SettingsError::FileReadError(error)),
                }
            },
            Ok(false) => {
                let settings = Settings::from_sensible_defaults();
                match settings.save(file) {
                    Ok(_) => Ok(settings),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(SettingsError::FileExistenceError(error)),
        }
    }

    pub fn from_sensible_defaults() -> Settings {
        Self {
            session_base_dir: Self::determine_session_base_dir(),
        }
    }

    pub fn determine_session_base_dir() -> PathBuf {
        // Get OS-specific document dir and create a directory named Hamshark
        UserDirs::new().map(|user_dirs| {
            user_dirs.document_dir().map(|doc_dir| {
                PathBuf::from(doc_dir).join(APPLICATION)
            })
        }).flatten().expect("Could not determine OS base dir")
    }

    pub fn save(&self, file: &Path) -> SettingsResult {
        match toml::to_string(self) {
            Ok(serialized) => {
                // If we can resolve a parent directory, create it.
                if let Some(parent) = file.parent() {
                    if let Err(error) = fs::create_dir_all(parent) {
                        return Err(SettingsError::DirectoryCreationError(error));
                    }
                };
                match fs::write(file, serialized) {
                    Ok(()) => Ok(()),
                    Err(error) => Err(SettingsError::FileWriteError(error)),
                }
            },
            Err(error) => Err(SettingsError::SerializationError(error)),
        }
    }
}