# Stscli

A command line application for acquiring and using Amazon STS session tokens with IAM roles.

## Usage

List the available profiles (from `.aws/config` and `.aws/credentials`):
```
stscli list
```

Acquire some tokens and display them for pasting into a bash shell:
```
stscli --profile foo get --export --format bash
```

Acquire some tokens and print the iam user using the aws cli:
```
stscli --profile foo exec aws iam get-user
```

You will want to configure at least a single set of credentials in ~/.aws/credentials

```
[myprofile]
aws_access_key_id=foo
aws_secret_access_key=bar
```

You can also add further profiles to ~/.aws/config
```
[profile myroleprofile]
source_profile=myprofile
role_arn=arn:aws:iam:....
```

You can then pass either "myprofile" or "myroleprofile" to the stscli as the profile to use.

You can override the role arn and region and also the role session name by passing additional options. See `--help`.

To use MFA with a role, use a command like the following:
```
stscli -p profile -s arn:aws:iam::999999999999:mfa/user -t 999999 exec -- aws ec2 describe-instances
```
Instead of the mfa arn you can also use a serial number.

There is a bash completion script. Ensure `stscli` is on your `PATH` and source this in your bash profile to get hints.
In particular you can press tab after the `--profile` option to get a list of available profiles.
