//! Runs the built binary against a local stand-in for STS, so the whole path
//! from flags to printed variables is exercised without an AWS account.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread;

const SECRET: &str = "secret/with+chars";
const TOKEN: &str = "token$with\"quote";

/// Answers every request with one set of credentials and keeps each request
/// body, so a test can see which STS action was called and with what.
struct StubSts {
    url: String,
    bodies: Arc<Mutex<Vec<String>>>,
}

impl StubSts {
    fn start() -> StubSts {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&bodies);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let body = String::from_utf8(body).unwrap();
                let action = if body.contains("Action=AssumeRole") {
                    "AssumeRole"
                } else {
                    "GetSessionToken"
                };
                seen.lock().unwrap().push(body);
                let xml = format!(
                    "<{action}Response xmlns=\"https://sts.amazonaws.com/doc/2011-06-15/\">\
                     <{action}Result><Credentials>\
                     <AccessKeyId>ASIA{action}</AccessKeyId>\
                     <SecretAccessKey>{SECRET}</SecretAccessKey>\
                     <SessionToken>token$with&quot;quote</SessionToken>\
                     <Expiration>2030-01-01T00:00:00Z</Expiration>\
                     </Credentials></{action}Result>\
                     <ResponseMetadata><RequestId>stub</RequestId></ResponseMetadata>\
                     </{action}Response>"
                );
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{xml}",
                    xml.len()
                );
            }
        });
        StubSts { url, bodies }
    }

    fn bodies(&self) -> Vec<String> {
        self.bodies.lock().unwrap().clone()
    }
}

/// A home directory with an AWS config and credentials file, and a command
/// that sees nothing of the real environment's AWS settings.
fn stscli(home: &Path, sts: &StubSts) -> Command {
    let aws = home.join(".aws");
    std::fs::create_dir_all(&aws).unwrap();
    std::fs::write(
        aws.join("credentials"),
        "[base]\naws_access_key_id = AKIABASE\naws_secret_access_key = basesecret\n",
    )
    .unwrap();
    std::fs::write(
        aws.join("config"),
        "[default]\nregion = eu-west-1\n\n\
         [profile admin]\nrole_arn = arn:aws:iam::123456789012:role/admin\nsource_profile = base\n",
    )
    .unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_stscli"));
    for (k, _) in std::env::vars() {
        if k.starts_with("AWS_") {
            cmd.env_remove(k);
        }
    }
    cmd.env("HOME", home)
        .env("USERPROFILE", home)
        .env("AWS_ENDPOINT_URL", &sts.url)
        .env("AWS_EC2_METADATA_DISABLED", "true");
    cmd
}

fn ok(output: Output) -> String {
    assert!(
        output.status.success(),
        "stscli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn get_session_token_as_json() {
    let home = tempfile::tempdir().unwrap();
    let sts = StubSts::start();
    let out = ok(stscli(home.path(), &sts)
        .args(["-p", "base", "get", "--format", "json"])
        .output()
        .unwrap());

    let vars: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(vars["AWS_ACCESS_KEY_ID"], "ASIAGetSessionToken");
    assert_eq!(vars["AWS_SECRET_ACCESS_KEY"], SECRET);
    assert_eq!(vars["AWS_SESSION_TOKEN"], TOKEN);
    assert_eq!(vars["AWS_SECURITY_TOKEN"], TOKEN);
    assert!(vars.get("AWS_DEFAULT_REGION").is_none());

    let bodies = sts.bodies();
    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].contains("Action=GetSessionToken"));
}

#[test]
fn role_profile_assumes_the_role_with_mfa() {
    let home = tempfile::tempdir().unwrap();
    let sts = StubSts::start();
    let out = ok(stscli(home.path(), &sts)
        .args([
            "--profile",
            "admin",
            "-n",
            "ci-session",
            "-s",
            "arn:aws:iam::123456789012:mfa/me",
            "-t",
            "123456",
            "-R",
            "us-west-2",
            "get",
            "--export",
        ])
        .output()
        .unwrap());

    assert!(out.contains("export AWS_ACCESS_KEY_ID=\"ASIAAssumeRole\"\n"));
    assert!(out.contains("export AWS_SESSION_TOKEN=\"token\\$with\\\"quote\"\n"));
    assert!(out.contains("export AWS_DEFAULT_REGION=\"us-west-2\"\n"));

    let body = &sts.bodies()[0];
    assert!(body.contains("Action=AssumeRole"));
    assert!(body.contains("RoleArn=arn%3Aaws%3Aiam%3A%3A123456789012%3Arole%2Fadmin"));
    assert!(body.contains("RoleSessionName=ci-session"));
    assert!(body.contains("SerialNumber=arn%3Aaws%3Aiam%3A%3A123456789012%3Amfa%2Fme"));
    assert!(body.contains("TokenCode=123456"));
}

#[cfg(unix)]
#[test]
fn exec_passes_the_credentials_and_the_exit_status() {
    let home = tempfile::tempdir().unwrap();
    let sts = StubSts::start();
    let out = ok(stscli(home.path(), &sts)
        .args([
            "-p",
            "base",
            "exec",
            "sh",
            "-c",
            "printf '%s %s' \"$AWS_ACCESS_KEY_ID\" \"$AWS_SESSION_TOKEN\"",
        ])
        .output()
        .unwrap());
    assert_eq!(out, format!("ASIAGetSessionToken {TOKEN}"));

    let status = stscli(home.path(), &sts)
        .args(["-p", "base", "exec", "--", "sh", "-c", "exit 3"])
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(3));
}

#[test]
fn list_merges_both_files() {
    let home = tempfile::tempdir().unwrap();
    let sts = StubSts::start();
    let out = ok(stscli(home.path(), &sts).arg("list").output().unwrap());
    assert_eq!(out, "admin\nbase\n");
    assert!(sts.bodies().is_empty());
}

#[test]
fn missing_named_config_file_is_an_error() {
    let home = tempfile::tempdir().unwrap();
    let sts = StubSts::start();
    let output = stscli(home.path(), &sts)
        .args(["-c", "/nonexistent/stscli-config", "-p", "base", "get"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("/nonexistent/stscli-config"));
}

#[test]
fn works_without_any_config_file() {
    let home = tempfile::tempdir().unwrap();
    let sts = StubSts::start();
    let mut cmd = stscli(home.path(), &sts);
    std::fs::remove_file(home.path().join(".aws/config")).unwrap();
    let out = ok(cmd
        .args(["-p", "base", "get", "-f", "json"])
        .output()
        .unwrap());
    assert!(out.contains("ASIAGetSessionToken"));
}
