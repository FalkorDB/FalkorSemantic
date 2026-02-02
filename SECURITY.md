# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take the security of FalkorSemantic seriously. If you believe you have found a security vulnerability, please report it to us as described below.

### Reporting Process

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them via email to: security@falkordb.com

You should receive a response within 48 hours. If for some reason you do not, please follow up via email to ensure we received your original message.

Please include the requested information listed below (as much as you can provide) to help us better understand the nature and scope of the possible issue:

* Type of issue (e.g., buffer overflow, SQL injection, cross-site scripting, etc.)
* Full paths of source file(s) related to the manifestation of the issue
* The location of the affected source code (tag/branch/commit or direct URL)
* Any special configuration required to reproduce the issue
* Step-by-step instructions to reproduce the issue
* Proof-of-concept or exploit code (if possible)
* Impact of the issue, including how an attacker might exploit the issue

This information will help us triage your report more quickly.

### Preferred Languages

We prefer all communications to be in English.

## Disclosure Policy

When we receive a security bug report, we will:

1. Confirm the problem and determine the affected versions
2. Audit code to find any similar problems
3. Prepare fixes for all supported releases
4. Release new versions as soon as possible

## Comments on This Policy

If you have suggestions on how this process could be improved, please submit a pull request.

## Security Update Policy

* Security updates will be released as soon as possible after a vulnerability is confirmed
* Updates will be backported to supported versions where feasible
* Security advisories will be published on GitHub Security Advisories

## Known Security Considerations

### Redis Module Security

As a Redis module, FalkorSemantic inherits Redis's security model:

* Always run Redis behind a firewall
* Use authentication when exposing Redis to untrusted networks
* Follow Redis security best practices: https://redis.io/topics/security

### Dependency Security

* We use `cargo-audit` to check for known vulnerabilities in dependencies
* Dependencies are regularly updated to address security issues
* CI pipeline includes automated security audits

## Best Practices

When using FalkorSemantic in production:

1. Keep Redis and FalkorDB updated to the latest stable versions
2. Use network segmentation and firewall rules
3. Enable Redis authentication (requirepass)
4. Disable dangerous Redis commands in production
5. Monitor logs for suspicious activity
6. Regularly backup your data
7. Review and test security updates before deploying to production

## Contact

For security-related questions or concerns, please contact:
* Email: security@falkordb.com
* GitHub: https://github.com/FalkorDB/FalkorSemantic/security
