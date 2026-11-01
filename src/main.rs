mod config;
mod print;
mod profiles;
mod result;
mod sts;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command as Process, ExitCode};

use clap::{Parser, Subcommand};
use log::debug;

use config::Config;
use print::Format;
use profiles::ProfileFile;
use result::{Result, StsCliError};

/// Acquire session tokens from Amazon STS.
#[derive(Debug, Parser)]
#[command(name = "stscli", version, author, arg_required_else_help = true)]
struct Cli {
    /// Sets a custom aws config file
    #[arg(short = 'c', long, value_name = "FILE", global = true)]
    config: Option<PathBuf>,

    /// Sets a custom aws credentials file
    #[arg(short = 'd', long, value_name = "FILE", global = true)]
    credentials: Option<PathBuf>,

    /// Select which profile to use from the config or credentials file
    #[arg(short = 'p', long, global = true)]
    profile: Option<String>,

    /// Set the arn of the role to assume
    #[arg(short = 'r', long, global = true)]
    role: Option<String>,

    /// Set the name of the region to use, e.g. eu-west-1
    #[arg(short = 'R', long, global = true)]
    region: Option<String>,

    /// The name of the session to use if assuming a role. It will appear in
    /// CloudTrail logs. [\w+=,.@-]*
    #[arg(short = 'n', long, global = true)]
    name: Option<String>,

    /// The serial number or ARN of the MFA device
    #[arg(short = 's', long, global = true)]
    serial_number: Option<String>,

    /// The code from the MFA device
    #[arg(short = 't', long, global = true)]
    token_code: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Get some fresh session tokens and display them
    Get {
        /// Print the variables for exporting to a shell
        #[arg(short, long)]
        export: bool,

        /// Format to use when printing the variables
        #[arg(short, long, value_enum, default_value_t = Format::Bash)]
        format: Format,
    },

    /// Run a command with session tokens injected into the environment
    Exec {
        /// The command and its arguments
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<OsString>,
    },

    /// List the available profiles
    List,
}

impl Cli {
    fn config(&self) -> Result<Config> {
        let config_file_named = self.config.is_some();
        let credentials_file_named = self.credentials.is_some();
        Ok(Config {
            config_file: match &self.config {
                Some(path) => path.clone(),
                None => Config::default_path("config", "AWS_CONFIG_FILE")?,
            },
            config_file_named,
            credentials_file: match &self.credentials {
                Some(path) => path.clone(),
                None => Config::default_path("credentials", "AWS_SHARED_CREDENTIALS_FILE")?,
            },
            credentials_file_named,
            profile: self.profile.clone(),
            role: self.role.clone(),
            region: self.region.clone(),
            name: self.name.clone(),
            serial_number: self.serial_number.clone(),
            token_code: self.token_code.clone(),
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    env_logger::init();
    let cli = Cli::parse();

    match run(cli).await {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "Error: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode> {
    let config = cli.config()?;
    debug!("config: {config:?}");

    match cli.command {
        Command::Get { export, format } => {
            let vars = sts::vars(&session(&config).await?, &config);
            print!("{}", print::render(&vars, format, export)?);
            Ok(ExitCode::SUCCESS)
        }
        Command::Exec { command } => {
            let vars = sts::vars(&session(&config).await?, &config);
            exec(&command, &vars)
        }
        Command::List => {
            list(&config)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

async fn session(config: &Config) -> Result<sts::Session> {
    let file = ProfileFile::load(&config.config_file, config.config_file_named)?;
    let plan = config::plan(config, &file);
    debug!("plan: {plan:?}");
    sts::session(config, &plan).await
}

/// Runs the command and exits with its status, so `stscli exec` can stand in
/// for the command in a script.
fn exec(
    command: &[OsString],
    vars: &std::collections::BTreeMap<String, String>,
) -> Result<ExitCode> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| StsCliError::Message("no command given".to_owned()))?;
    let status = Process::new(program).args(args).envs(vars).status()?;
    match status.code() {
        Some(code) => Ok(ExitCode::from(u8::try_from(code).unwrap_or(1))),
        None => Err(StsCliError::Message(format!(
            "{} was killed by a signal",
            program.to_string_lossy()
        ))),
    }
}

/// Every profile named in either file, once each, sorted. The bash
/// completion script reads this.
fn list(config: &Config) -> Result<()> {
    let credentials = ProfileFile::load(&config.credentials_file, config.credentials_file_named)?;
    let profiles = ProfileFile::load(&config.config_file, config.config_file_named)?;
    let names: BTreeSet<&String> = credentials
        .profiles
        .keys()
        .chain(profiles.profiles.keys())
        .collect();
    for name in names {
        println!("{name}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn flags_keep_their_old_spellings() {
        let cli = Cli::try_parse_from([
            "stscli",
            "-c",
            "cfg",
            "-d",
            "creds",
            "-p",
            "prof",
            "-r",
            "arn",
            "-R",
            "eu-west-1",
            "-n",
            "name",
            "-s",
            "serial",
            "-t",
            "123456",
            "get",
            "-e",
            "-f",
            "fish",
        ])
        .unwrap();
        assert_eq!(cli.profile.as_deref(), Some("prof"));
        assert_eq!(cli.serial_number.as_deref(), Some("serial"));
        assert_eq!(cli.token_code.as_deref(), Some("123456"));
        assert!(matches!(
            cli.command,
            Command::Get {
                export: true,
                format: Format::Fish
            }
        ));

        let long = Cli::try_parse_from([
            "stscli",
            "--serial-number",
            "s",
            "--token-code",
            "t",
            "list",
        ])
        .unwrap();
        assert_eq!(long.serial_number.as_deref(), Some("s"));
    }

    #[test]
    fn exec_takes_the_rest_of_the_line() {
        let cli = Cli::try_parse_from([
            "stscli",
            "-p",
            "x",
            "exec",
            "aws",
            "s3",
            "ls",
            "--recursive",
        ])
        .unwrap();
        let Command::Exec { command } = cli.command else {
            panic!()
        };
        assert_eq!(command, ["aws", "s3", "ls", "--recursive"]);

        let dashed = Cli::try_parse_from(["stscli", "exec", "--", "aws", "--version"]).unwrap();
        let Command::Exec { command } = dashed.command else {
            panic!()
        };
        assert_eq!(command, ["aws", "--version"]);
    }

    #[test]
    fn unknown_format_is_rejected() {
        assert!(Cli::try_parse_from(["stscli", "get", "-f", "zsh"]).is_err());
    }
}
