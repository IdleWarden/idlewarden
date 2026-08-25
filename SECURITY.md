# Security policy

## Reporting

Do **not** open a public issue for a vulnerability. Use GitHub's private
[security advisory](https://github.com/idlewarden/idlewarden/security/advisories/new)
form. Expect a first response within a week.

## What counts

* Anything letting a plugin escape its declared capabilities.
* Anything letting a registry entry cause code execution on install.
* Signature or checksum verification that can be bypassed.
* Path traversal when extracting a plugin package.

## What does not

* "The app can control the mouse and read the screen." That is the product.
* Automation being detectable by a game. Out of scope by design — we are not in
  the business of hiding.

## Antivirus false positives

IdleWarden captures the screen, synthesises input and enumerates processes. That
is the heuristic signature of a remote access trojan, and false positives are
expected rather than surprising.

Our answers, in order: the source is public, releases are built in CI from a
tagged commit, binaries are signed, and checksums are published. If your scanner
flags a **signed release binary**, please tell us — we submit those for
reclassification.

We will never ask you to disable your antivirus.
