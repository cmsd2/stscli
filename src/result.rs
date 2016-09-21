use rusoto;
use std::io;
use std::result;
use rustc_serialize::json;

quick_error! {
    #[derive(Debug)]
    pub enum StsCliError {
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
    }
}

pub type Result<T> = result::Result<T, StsCliError>;
