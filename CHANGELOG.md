# Changelog

Releases before 1.0.9 are listed on the [releases page](https://github.com/handsomefox/win-cleaner/releases).

## Unreleased

- Show an app with one cleanup target as a single row that holds the app name, the target, and
  its size. It used to take a header row with a "0/1 selected" count and a second row for the
  target.
- Lay app cards out in up to four columns when the window is wide enough. A name or target
  too long for its card ends in an ellipsis and shows in full on hover.

## 1.0.9

- Ship `win-cleaner-<version>-windows-x86_64.zip`, which holds a folder of the same name with
  `win-cleaner.exe`, `README.md`, and `LICENSE` in it, beside a `SHA256SUMS` file. The
  executable used to be `Windows Cleaner.exe`, and was also attached on its own. Windows still
  shows it as Windows Cleaner.
- Show the release version in the executable's properties. Every release after 1.0.4 still
  said 1.0.4 there.
