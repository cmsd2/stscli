# Stscli

A command line application for acquiring and using Amazon STS session tokens with IAM roles.

## Usage

Acquire some tokens and display them:
```
stscli --profile foo get
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

You can override the role arn and region and also the role session name by passing additional options. See --help.

