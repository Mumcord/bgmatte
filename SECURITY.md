# Security policy

bgmatte is part of the Mumcord project. The [Mumcord security policy](https://github.com/Mumcord/mumcord/blob/main/SECURITY.md) applies to it in full: what to expect after a report, disclosure, and the rules for testing.

## Supported versions

Security fixes are made for the **latest release** and ship as a new patch release. Older releases do not get fixes; please update to the latest release.

## Reporting a vulnerability

**Do not report vulnerabilities in public issues, pull requests or discussions.**

Report privately through GitHub:

**<https://github.com/Mumcord/bgmatte/security/advisories/new>**

Only you and the maintainers can read the report. If you cannot use this form, open a normal issue that only says you would like to report a security problem and asks for a private channel. Do not include any details in it.

A good report contains the bgmatte and ONNX Runtime versions, your operating system, what an attacker can achieve and what they need for it (for example: a crafted background image file), and steps to reproduce.

## What counts as a vulnerability

- A crash, out-of-bounds access or unbounded memory use reachable through the public interface, for example through a crafted background image or unusual frame sizes.
- Loading or running code from somewhere other than the ONNX Runtime library the application chose.

Problems in ONNX Runtime itself or in the image decoders are best reported to their own projects as well; tell us too, so that a release that avoids them can follow.
