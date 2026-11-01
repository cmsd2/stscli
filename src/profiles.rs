//! Reads the profile names, regions and role settings stscli needs out of the
//! AWS config and credentials files. Credentials themselves are left to the
//! SDK's provider chain.

use std::collections::BTreeMap;
use std::path::Path;

use ini::Ini;

use crate::result::{Result, StsCliError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    pub role_arn: Option<String>,
    pub source_profile: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileFile {
    pub default_region: Option<String>,
    pub profiles: BTreeMap<String, Profile>,
}

impl ProfileFile {
    /// Loads a file. A missing file reads as empty when `required` is false,
    /// so a default location that does not exist is not an error.
    pub fn load(path: &Path, required: bool) -> Result<ProfileFile> {
        if !required && !path.exists() {
            return Ok(ProfileFile::default());
        }
        let ini = Ini::load_from_file(path).map_err(|source| StsCliError::Ini {
            path: path.to_owned(),
            source,
        })?;
        Ok(ProfileFile::from_ini(&ini))
    }

    #[cfg(test)]
    pub fn parse(text: &str) -> Result<ProfileFile> {
        let ini = Ini::load_from_str(text).map_err(|e| StsCliError::Ini {
            path: "<string>".into(),
            source: ini::Error::Parse(e),
        })?;
        Ok(ProfileFile::from_ini(&ini))
    }

    fn from_ini(ini: &Ini) -> ProfileFile {
        let default_region = ini
            .section(Some("default"))
            .and_then(|s| s.get("region"))
            .map(str::to_owned);

        let mut profiles = BTreeMap::new();
        for (name, section) in ini.iter() {
            let Some(profile_name) = name.and_then(profile_name) else {
                continue;
            };
            profiles.insert(
                profile_name,
                Profile {
                    role_arn: section.get("role_arn").map(str::to_owned),
                    source_profile: section.get("source_profile").map(str::to_owned),
                    region: section.get("region").map(str::to_owned),
                },
            );
        }

        ProfileFile {
            default_region,
            profiles,
        }
    }
}

/// The profile a section names: `[profile foo]` in the config file and
/// `[foo]` in the credentials file are both `foo`. `[default]` names none.
pub fn profile_name(section: &str) -> Option<String> {
    if let Some(name) = section.strip_prefix("profile ") {
        Some(name.trim().to_owned())
    } else if section != "default" {
        Some(section.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_names() {
        assert_eq!(profile_name("profile admin"), Some("admin".to_owned()));
        assert_eq!(profile_name("admin"), Some("admin".to_owned()));
        assert_eq!(profile_name("default"), None);
    }

    #[test]
    fn parses_regions_and_roles() {
        let file = ProfileFile::parse(
            "[default]\nregion = eu-west-1\n\n\
             [profile admin]\nrole_arn = arn:aws:iam::123456789012:role/admin\n\
             source_profile = base\nregion = us-west-2\n\n\
             [profile plain]\n",
        )
        .unwrap();

        assert_eq!(file.default_region.as_deref(), Some("eu-west-1"));
        assert_eq!(
            file.profiles["admin"],
            Profile {
                role_arn: Some("arn:aws:iam::123456789012:role/admin".to_owned()),
                source_profile: Some("base".to_owned()),
                region: Some("us-west-2".to_owned()),
            }
        );
        assert_eq!(file.profiles["plain"], Profile::default());
        assert!(!file.profiles.contains_key("default"));
    }

    #[test]
    fn missing_optional_file_is_empty() {
        let file = ProfileFile::load(Path::new("/nonexistent/stscli/config"), false).unwrap();
        assert_eq!(file, ProfileFile::default());
    }

    #[test]
    fn missing_required_file_is_an_error() {
        let err = ProfileFile::load(Path::new("/nonexistent/stscli/config"), true).unwrap_err();
        assert!(err.to_string().contains("/nonexistent/stscli/config"));
    }
}
