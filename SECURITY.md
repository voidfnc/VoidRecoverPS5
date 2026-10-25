# Security

Please report security problems privately to the maintainer through GitHub (the repository's "Report a vulnerability" button once it is enabled) rather than in a public issue.

Design rules this project keeps:

- The source registry is data only and is compiled into the app. It can't contain commands, scripts or hooks, and it changes only with an app update.
- The UI can't run shell commands. File and network access happens in fixed Rust commands; there is no shell, filesystem or HTTP plugin in the webview.
- Network requests are https only, including every redirect hop. External links open only in the system browser, after a prompt that names the domain.
- Firmware files are only read and hashed. They are never executed or unpacked.
- The app never formats, erases or repartitions drives.

Never attach firmware files or other copyrighted content to issues.
