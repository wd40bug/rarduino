use core::error;
use glob::glob;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::DirEntry;
use std::path::PathBuf;
use std::{fs, io};

#[derive(Debug, Deserialize)]
pub struct BindgenLists {
  pub allowlist_function: Vec<String>,
  pub allowlist_type: Vec<String>,
  pub blocklist_function: Vec<String>,
  pub blocklist_type: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigSerialize {
  /// Path to the arduino home directory
  /// Usuall $HOME/.arduino15
  pub arduino_home: PathBuf,
  /// Path to the arduino external libraries directory
  /// Usually $HOME/Arduino
  pub external_libraries_home: PathBuf,
  /// Core version
  /// Usually 1.8.6
  pub core_version: String,
  /// Variant
  /// Usually eightanaloginputs
  pub variant: String,
  /// Avr Gcc Verion
  /// Usually 7.3.0-atmel3.6.1-arduino7
  pub avr_gcc_version: String,
  /// List of arduino libraries to use
  pub arduino_libraries: Vec<String>,
  /// List of external libraries to use
  pub external_libraries: Vec<String>,
  /// List of definitions
  /// Usually:
  /// DUINO: '10807'
  /// F_CPU: 16000000L
  /// ARDUINO_AVR_UNO: '1'
  /// ARDUINO_ARCH_AVR: '1'
  pub definitions: HashMap<String, String>,
  /// List of compile flags
  /// Usually:
  /// '-mmcu=atmega328p'
  pub flags: Vec<String>,
  /// List of allowed and blocked functions and types
  pub bindgen_lists: BindgenLists,
}

struct Config {
  /// List of home directories for includes
  includes: Vec<PathBuf>,
  /// Path to avr_gcc binary
  avr_gcc: PathBuf,
  /// List of all cpp files
  cpp_files: Vec<PathBuf>,
  /// List of all c files
  c_files: Vec<PathBuf>,
}

impl TryFrom<ConfigSerialize> for Config {
  type Error = ConfigError;

  fn try_from(value: ConfigSerialize) -> Result<Self, Self::Error> {
    let arduino_home_str = value
      .arduino_home
      .to_str()
      .ok_or(ConfigError::ArduinoHomeNoString(value.arduino_home.clone()))?;
    let external_libraries_home_str =
      value
        .external_libraries_home
        .to_str()
        .ok_or(ConfigError::ExternalLibrariesHomeNoString(
          value.external_libraries_home.clone(),
        ))?;
    let arduino_home = PathBuf::from(envmnt::expand(arduino_home_str, None)); // Location to search for Arduino libraries
    let external_libraries_home = PathBuf::from(envmnt::expand(external_libraries_home_str, None)); // Location to search for External Libraries
    if !arduino_home.exists() {
      return Err(ConfigError::ArduinoHomeNoExist(arduino_home));
    }
    if !external_libraries_home.exists() {
      return Err(ConfigError::ExternalLibrariesHomeNoExist(
        external_libraries_home,
      ));
    }
    //TODO: Verify assumed structure
    let arduino_package_path = arduino_home.join("packaged").join("arduino");
    let avr_gcc_home = arduino_package_path
      .join("tools")
      .join("avr-gcc")
      .join(value.avr_gcc_version);
    let core_path = arduino_package_path
      .join("hardware")
      .join("avr")
      .join(&value.core_version);
    let avr_gcc_bin = avr_gcc_home.join("bin").join("avr-gcc");
    if !avr_gcc_bin.exists() {
      return Err(ConfigError::NoAvrGcc(avr_gcc_bin));
    }

    let arduino_includes = [
      core_path
        .join("hardware")
        .join("avr")
        .join(&value.core_version), // Path to the arduino core
      core_path.join("variants").join(&value.variant), // Path to the arduino variant code
      avr_gcc_home.join("include"),                    // avr-gcc includes
    ];
    let arduino_libraries: Vec<PathBuf> = {
      let library_path = core_path.join("libraries");
      value
        .arduino_libraries
        .iter()
        .map(|lib| src_root(&library_path.join(lib)))
        .collect::<Result<Vec<PathBuf>, ConfigError>>()?
    };
    let external_libraries: Vec<PathBuf> = value
      .external_libraries
      .iter()
      .map(|lib| src_root(&external_libraries_home.join(lib)))
      .collect::<Result<Vec<PathBuf>, ConfigError>>()?;
    let mut include_dirs = Vec::from(arduino_includes);
    include_dirs.extend(arduino_libraries);
    include_dirs.extend(external_libraries);

    let get_type = |pattern: &str| -> Result<Vec<PathBuf>, ConfigError> {
      let mut result = Vec::new();
      for file in &include_dirs {
        let files = glob(&format!(
          "{}/**/{}",
          file
            .to_str()
            .ok_or(ConfigError::ConvertFailed(file.clone()))?,
          pattern
        ))?
        .filter_map(|f| -> Option<Result<PathBuf, ConfigError>> {
          let path = match f {
            Ok(path) => path,
            Err(e) => return Some(Err(e.into())),
          };
          if path.ends_with("main.cpp") {
            None
          } else {
            Some(Ok(path))
          }
        }).collect::<Result<Vec<PathBuf>, ConfigError>>()?;
        result.extend(files);
      }
      Ok(result)
    };
    todo!()
  }
}

fn src_root(loc: &PathBuf) -> Result<PathBuf, ConfigError> {
  let children: Vec<PathBuf> = fs::read_dir(loc)?
    .collect::<io::Result<Vec<DirEntry>>>()?
    .into_iter()
    .map(|x| x.path())
    .collect();
  let src_path = loc.join("./src");
  let utility_path = loc.join("./utility");
  let src = children.contains(&src_path);
  let utility = children.contains(&utility_path);
  match (src, utility) {
    (true, true) => Err(ConfigError::MalformedLib(loc.clone())),
    (true, false) => Ok(src_path),
    (false, true) => Ok(utility_path),
    (false, false) => Ok(loc.clone()),
  }
}

fn compile(config: &Config) {}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
  #[error("The provided path cannot be converted to UTF-8: {}", .0.to_string_lossy())]
  ConvertFailed(PathBuf),
  #[error("The provided arduino home is not valid UTF-8: {}", .0.to_string_lossy())]
  ArduinoHomeNoString(PathBuf),
  #[error("The provided external libraries home is not valid UTF-8: {}", .0.to_string_lossy())]
  ExternalLibrariesHomeNoString(PathBuf),
  #[error("The provided arduino home does not exist: {}", .0.to_string_lossy())]
  ArduinoHomeNoExist(PathBuf),
  #[error("The provided external libraries home does not exist: {}", .0.to_string_lossy())]
  ExternalLibrariesHomeNoExist(PathBuf),
  #[error("Couldn't find avr-gcc at {}", .0.to_string_lossy())]
  NoAvrGcc(PathBuf),
  #[error("malformed library, expected one of 'utility', 'src', or neither: {}", .0.to_string_lossy())]
  MalformedLib(PathBuf),
  #[error("failed during a file operation: {0}")]
  Io(#[from] io::Error),
  #[error("failed during a glob pattern operation: {0}")]
  GlobPatternError(#[from] glob::PatternError),
  #[error("failed during a glob iteration operation: {0}")]
  GlobIterationError(#[from] glob::GlobError),
}

#[cfg(test)]
mod tests {
  use super::*;
}
