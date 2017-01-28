use rusoto;
use std::io;
use std::result;
use rustc_serialize::json;
use ini::ini;

quick_error! {
    #[derive(Debug)]
    pub enum StsCliError {
        Error(descr: String) {
            description("error")
            display("error: {}", descr)
        }

        Io(err: io::Error) {
            from()
            description("io error")
            display("I/O error: {}", err)
            cause(err)
        }

        Credentials(err: rusoto::CredentialsError) {
            from()
            description("aws credentials error")
            display("AWS Credentials error: {}", err)
            cause(err)
        }

        Region(err: rusoto::ParseRegionError) {
            from()
            description("aws region error")
            display("AWS Region parser error: {}", err)
            cause(err)
        }

        ProcessKilled {
            description("process killed")
            display("process killed")
        }

        ChildExited(code: i32) {
            description("child exited")
            display("child exited: {}", code)
        }

        JsonEncoderError(err: json::EncoderError) {
            from()
            description("json encoder error")
            display("Json encoder error: {}", err)
            cause(err)
        }

        IniError(err: ini::Error) {
            from()
            description("ini error")
            display("ini error: {}", err)
            cause(err)
        }

        AssumeRoleError(err: rusoto::sts::AssumeRoleError) {
            from()
            description("STS AssumeRoleError")
            display("STS AssumeRoleError: {}", err)
            cause(err)
        }

        GetSessionTokenError(err: rusoto::sts::GetSessionTokenError) {
            from()
            description("STS GetSessionTokenError")
            display("STS GetSessionTokenError: {}", err)
            cause(err)
        }

        TlsError(err: rusoto::TlsError) {
            from()
            description("TLS Error")
            display("TLS Error: {}", err)
            cause(err)
        }
    }
}

pub type Result<T> = result::Result<T, StsCliError>;
