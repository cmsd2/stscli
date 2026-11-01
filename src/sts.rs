//! The STS call: AssumeRole when there is a role, GetSessionToken otherwise,
//! signed with the base profile's credentials.

use std::collections::BTreeMap;

use aws_config::{BehaviorVersion, Region};
use aws_runtime::env_config::file::{EnvConfigFileKind, EnvConfigFiles};
use aws_sdk_sts::error::DisplayErrorContext;
use aws_sdk_sts::types::Credentials;

use crate::config::{Config, Plan};
use crate::result::{Result, StsCliError};

pub struct Session {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: String,
}

pub async fn session(config: &Config, plan: &Plan) -> Result<Session> {
    // A file the user named is passed as is, and must exist. Otherwise the
    // SDK looks in its default place, where a missing file is not an error.
    let mut files = EnvConfigFiles::builder();
    files = if config.config_file_named {
        files.with_file(EnvConfigFileKind::Config, &config.config_file)
    } else {
        files.include_default_config_file(true)
    };
    files = if config.credentials_file_named {
        files.with_file(EnvConfigFileKind::Credentials, &config.credentials_file)
    } else {
        files.include_default_credentials_file(true)
    };
    let files = files.build();

    // The loader's default credentials chain reads the environment, then the
    // profile files, then container and instance credentials: the same order
    // the AWS CLI uses. The region is ours, not the profile's.
    let mut loader = aws_config::defaults(BehaviorVersion::latest())
        .profile_files(files)
        .region(Region::new(plan.region.clone()));
    if let Some(profile) = &plan.base_profile {
        loader = loader.profile_name(profile);
    }
    let sdk = loader.load().await;
    let client = aws_sdk_sts::Client::new(&sdk);

    let credentials = match &plan.role_arn {
        Some(role_arn) => {
            client
                .assume_role()
                .role_arn(role_arn)
                .role_session_name(&plan.session_name)
                .set_serial_number(config.serial_number.clone())
                .set_token_code(config.token_code.clone())
                .send()
                .await
                .map_err(|e| sts_error("AssumeRole", e))?
                .credentials
        }
        None => {
            client
                .get_session_token()
                .set_serial_number(config.serial_number.clone())
                .set_token_code(config.token_code.clone())
                .send()
                .await
                .map_err(|e| sts_error("GetSessionToken", e))?
                .credentials
        }
    };

    let c: Credentials = credentials
        .ok_or_else(|| StsCliError::Message("STS returned no credentials".to_owned()))?;
    Ok(Session {
        access_key_id: c.access_key_id().to_owned(),
        secret_access_key: c.secret_access_key().to_owned(),
        session_token: c.session_token().to_owned(),
    })
}

fn sts_error<E: std::error::Error>(operation: &'static str, e: E) -> StsCliError {
    StsCliError::Sts {
        operation,
        message: DisplayErrorContext(e).to_string(),
    }
}

/// The variables `get` prints and `exec` sets. AWS_SECURITY_TOKEN is the
/// older name some tools still read. AWS_DEFAULT_REGION is set only when
/// `--region` asked for one.
pub fn vars(session: &Session, config: &Config) -> BTreeMap<String, String> {
    let mut env = BTreeMap::from([
        (
            "AWS_ACCESS_KEY_ID".to_owned(),
            session.access_key_id.clone(),
        ),
        (
            "AWS_SECRET_ACCESS_KEY".to_owned(),
            session.secret_access_key.clone(),
        ),
        (
            "AWS_SESSION_TOKEN".to_owned(),
            session.session_token.clone(),
        ),
        (
            "AWS_SECURITY_TOKEN".to_owned(),
            session.session_token.clone(),
        ),
    ]);
    if let Some(region) = &config.region {
        env.insert("AWS_DEFAULT_REGION".to_owned(), region.clone());
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            access_key_id: "AKID".to_owned(),
            secret_access_key: "SECRET".to_owned(),
            session_token: "TOKEN".to_owned(),
        }
    }

    #[test]
    fn vars_without_region() {
        let v = vars(&session(), &Config::default());
        assert_eq!(
            v.keys().collect::<Vec<_>>(),
            [
                "AWS_ACCESS_KEY_ID",
                "AWS_SECRET_ACCESS_KEY",
                "AWS_SECURITY_TOKEN",
                "AWS_SESSION_TOKEN"
            ]
        );
        assert_eq!(v["AWS_SECURITY_TOKEN"], "TOKEN");
    }

    #[test]
    fn vars_with_region() {
        let c = Config {
            region: Some("eu-west-1".to_owned()),
            ..Config::default()
        };
        assert_eq!(vars(&session(), &c)["AWS_DEFAULT_REGION"], "eu-west-1");
    }
}
