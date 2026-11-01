//! What a run asks for, and what that resolves to once the config file has
//! had its say: which region, which role, and whose credentials to start from.

use std::path::PathBuf;

use crate::profiles::ProfileFile;
use crate::result::{Result, StsCliError};

pub const DEFAULT_REGION: &str = "us-east-1";
pub const DEFAULT_SESSION_NAME: &str = "stscli";

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// The config file, and whether the user named it. A named file must
    /// exist; the default location may not.
    pub config_file: PathBuf,
    pub config_file_named: bool,
    pub credentials_file: PathBuf,
    pub credentials_file_named: bool,
    pub profile: Option<String>,
    pub role: Option<String>,
    pub region: Option<String>,
    pub name: Option<String>,
    pub serial_number: Option<String>,
    pub token_code: Option<String>,
}

impl Config {
    /// Where a file lives when no flag names it: the variable the SDK also
    /// reads, or `~/.aws/<file>`. Keeping to the SDK's rule means the region
    /// read here and the credentials it reads come from the same file.
    pub fn default_path(file: &str, variable: &str) -> Result<PathBuf> {
        if let Some(path) = std::env::var_os(variable).filter(|p| !p.is_empty()) {
            return Ok(PathBuf::from(path));
        }
        std::env::home_dir()
            .map(|home| home.join(".aws").join(file))
            .ok_or_else(|| StsCliError::Message("can't find the home directory".to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub region: String,
    pub role_arn: Option<String>,
    /// The profile whose credentials sign the STS call. For a role profile
    /// this is its `source_profile`.
    pub base_profile: Option<String>,
    pub session_name: String,
}

/// Region precedence, lowest first: us-east-1, the config file's default
/// region, the profile's region, `--region`. The role comes from the profile
/// unless `--role` overrides it.
pub fn plan(config: &Config, file: &ProfileFile) -> Plan {
    let mut region = file
        .default_region
        .clone()
        .unwrap_or_else(|| DEFAULT_REGION.to_owned());
    let mut role_arn = None;
    let mut base_profile = config.profile.clone();

    if let Some(name) = &config.profile
        && let Some(profile) = file.profiles.get(name)
    {
        if let Some(r) = &profile.region {
            region = r.clone();
        }
        role_arn = profile.role_arn.clone();
        if let Some(source) = &profile.source_profile {
            base_profile = Some(source.clone());
        }
    }

    if config.role.is_some() {
        role_arn = config.role.clone();
    }
    if let Some(r) = &config.region {
        region = r.clone();
    }

    Plan {
        region,
        role_arn,
        base_profile,
        session_name: config
            .name
            .clone()
            .unwrap_or_else(|| DEFAULT_SESSION_NAME.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> ProfileFile {
        ProfileFile::parse(
            "[default]\nregion = eu-west-1\n\n\
             [profile admin]\nrole_arn = arn:aws:iam::123456789012:role/admin\n\
             source_profile = base\nregion = us-west-2\n\n\
             [profile regional]\nregion = ap-south-1\n",
        )
        .unwrap()
    }

    fn config(profile: Option<&str>) -> Config {
        Config {
            profile: profile.map(str::to_owned),
            ..Config::default()
        }
    }

    #[test]
    fn no_file_no_profile() {
        let p = plan(&config(None), &ProfileFile::default());
        assert_eq!(p.region, "us-east-1");
        assert_eq!(p.role_arn, None);
        assert_eq!(p.base_profile, None);
        assert_eq!(p.session_name, "stscli");
    }

    #[test]
    fn default_region_from_file() {
        assert_eq!(plan(&config(None), &file()).region, "eu-west-1");
    }

    #[test]
    fn role_profile_signs_with_its_source() {
        let p = plan(&config(Some("admin")), &file());
        assert_eq!(p.region, "us-west-2");
        assert_eq!(
            p.role_arn.as_deref(),
            Some("arn:aws:iam::123456789012:role/admin")
        );
        assert_eq!(p.base_profile.as_deref(), Some("base"));
    }

    #[test]
    fn profile_without_source_signs_with_itself() {
        let p = plan(&config(Some("regional")), &file());
        assert_eq!(p.region, "ap-south-1");
        assert_eq!(p.role_arn, None);
        assert_eq!(p.base_profile.as_deref(), Some("regional"));
    }

    #[test]
    fn flags_override_the_file() {
        let c = Config {
            profile: Some("admin".to_owned()),
            role: Some("arn:aws:iam::123456789012:role/other".to_owned()),
            region: Some("eu-central-1".to_owned()),
            name: Some("me".to_owned()),
            ..Config::default()
        };
        let p = plan(&c, &file());
        assert_eq!(p.region, "eu-central-1");
        assert_eq!(
            p.role_arn.as_deref(),
            Some("arn:aws:iam::123456789012:role/other")
        );
        assert_eq!(p.base_profile.as_deref(), Some("base"));
        assert_eq!(p.session_name, "me");
    }
}
