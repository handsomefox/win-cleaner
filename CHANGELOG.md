# Changelog

Releases before 1.0.9 are listed on the [releases page](https://github.com/handsomefox/win-cleaner/releases).

## Unreleased

- Keep remembering a target through a run where it is empty or its app is
  gone. 1.0.12 dropped it from the saved selection instead, so a target you
  cleaned in one run came back unchecked two runs later even once it had data
  again. An empty target is still never checked for you, because there is
  nothing to free; the choice now waits for the run that has something to
  offer.
- Check the catalog's own picks again on a first launch. 1.0.12 applied an
  empty remembered selection over them, so a new install opened with nothing
  selected.

## 1.0.12

- Remember which targets you selected, and whether empty ones were listed, so the next launch
  starts where you left off. Targets are matched by app and name, and anything no longer on the
  machine is skipped. Preview only stays on at every launch and empty-folder removal stays
  opt-in per run.
- Add a Settings entry to the sidebar for those two preferences, with a button that forgets the
  saved selection.

## 1.0.11

- Offer the old versions that self-updating apps leave on disk: Discord's previous `app-*`
  build and the package it was installed from, Battle.net's superseded `Agent.NNNN`, and
  osu!lazer's previous package. The newest version is always kept, a name without a version
  number is never touched, and none of these targets is selected for you.
- Clean Discord's folder of downloaded updates.
- Find roughly 700 MB more on a typical install. The NVIDIA shader cache in `LocalLow` and the
  Ubisoft Connect cache under your profile were both missed: the launcher cache was only ever
  looked for in its install folder. Vortex's `temp`, qBittorrent's `cache`, and OBS Studio's
  `updates` folders are covered now too.
- Support three more apps: DLSS Updater, Cloudflare WARP, and VLC.
- Give every cleanup target one kind of data and one plain name. Cache, logs, crash reports,
  and downloaded updates used to share a row labelled `cache + logs` or
  `cache/media/temp/dumps`; each is now its own row, so you can clear a cache and keep the
  logs. The list holds 123 targets instead of 90.

## 1.0.10

- Show an app with one cleanup target as a single row that holds the app name, the target, and
  its size. It used to take a header row with a "0/1 selected" count and a second row for the
  target.
- Lay app cards out in up to four columns when the window is wide enough. A name or target
  too long for its card ends in an ellipsis and shows in full on hover.
- Give each category its own color. It tints the category icon in the sidebar and above the
  cards, and runs down the left edge of each app card. No category is red, which still marks
  large sizes and errors.
- Show the three sort orders as toolbar buttons instead of hiding them in a drop-down. A button
  next to the search box clears it.
- Collapse a category section to its header with the arrow beside its checkbox, or by clicking
  its name. **Collapse all** and **Expand all** in the toolbar do the same for every section in
  view. The select-all checkbox still covers items in collapsed sections.

## 1.0.9

- Ship `win-cleaner-<version>-windows-x86_64.zip`, which holds a folder of the same name with
  `win-cleaner.exe`, `README.md`, and `LICENSE` in it, beside a `SHA256SUMS` file. The
  executable used to be `Windows Cleaner.exe`, and was also attached on its own. Windows still
  shows it as Windows Cleaner.
- Show the release version in the executable's properties. Every release after 1.0.4 still
  said 1.0.4 there.
