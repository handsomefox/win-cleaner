# Changelog

Releases before 1.0.9 are listed on the [releases page](https://github.com/handsomefox/win-cleaner/releases).

## 1.4.0

- Picking a preset from **Presets** or **Manage presets** now checks its empty-folder rows. Saving
  a preset used to drop them. The preset marked **Use on start** and your last selection still
  leave them unchecked, so removing empty folders stays a choice you make each run.
- Presets saved before 1.4.0 hold no empty-folder rows. To add them, pick the preset, check the
  rows, then press **Update** beside it in **Manage presets**.
- Add **Update** to each preset in **Manage presets**. It stores the targets checked now and keeps
  the preset's name and **Use on start**. A target this scan could not offer, such as one with
  nothing to clean or whose program is running, stays in the preset.
- Hover a preset in the menu to see what it checks on this scan, and open **Manage presets** to see
  every target in each one. A target the preset leaves unchecked is greyed out with the reason.
- Empty-folder rows read "4 empty folders" instead of "4 items", and hovering the count lists the
  folders. Their details explain that a folder is removed only while it is still empty, and
  **Show files** no longer appears on these rows.
- Restarting with administrator rights keeps empty-folder rows checked.

## 1.3.1

- A target that could not be read, such as `Prefetch` without administrator rights, stays listed
  with its error even when empty targets are hidden. A select-all box leaves it unchecked, since
  cleaning it would only fail again.
- A target that Stop reached before anything in it moved reads **Stopped** in the results and in
  **History**, and no longer counts as skipped.
- With an age or size limit set, a target's file list says its files move one by one and its
  folders stay.
- The file list opens in the middle of the window, not where an earlier one was.

## 1.3.0

- Add **Exclusions** to **Settings**. Pick a folder or a file, or press **Exclude** in a target's
  file list, and Windows Cleaner never moves it. A folder that holds an exclusion is cleaned around
  it, never moved whole.
- Add two limits under **What to leave** in **Settings**: leave files changed in the last so many
  days, and leave files smaller than a size. With a limit set, targets are cleaned file by file and
  their folders stay. Old program versions are always moved whole or not at all.
- Add **Skip targets while their program is running** to **Settings**. A target whose program is
  open carries a **Running** badge and is not selected after a scan. If the program starts before
  the cleanup reaches the target, the target is skipped.
- Every cleanup checks each target against your exclusions and limits as they are when it runs,
  so an exclusion you add after a scan still protects its path.
- Add **Stop** to a running cleanup. It finishes the item being moved, then stops, and **History**
  records that the run was stopped. What already moved stays in the Recycle Bin.
- Add **Show files** to each target's details. It lists every file cleaning the target would move,
  sorted by path, size, or date, and stays smooth with hundreds of thousands of files.
- Add **Export settings** and **Import settings** to **Settings**, to keep a copy of your
  exclusions, limits, presets, and selection preferences or move them to another PC. An import
  checks the whole file before it applies anything.
- Mark targets under Program Files or Windows with a **Needs admin** badge, and add **Scan again as
  administrator**, which restarts Windows Cleaner with administrator rights and keeps your
  selection. **Always start with administrator rights** in **Settings** asks for them at every
  start. A settings file never carries that setting.
- If Windows gives Windows Cleaner administrator rights through a different account from the one
  signed in, it cleans nothing and says why, because the folders it would find belong to that
  other account. It also cleans nothing when it cannot confirm which account it runs as.
- Report a folder that cannot be read, such as `Prefetch` without administrator rights, as a
  failure instead of as empty.
- Group **Chrome Beta**, **Chrome Dev**, and **Chrome Canary** under **Chrome**, with one
  **Profile cache** target per channel, and add the same targets for Edge's Beta, Dev, and Canary
  channels. Selections and presets that name the old entries carry over.
- List the targets within each app in a fixed order, such as Stable, Beta, Dev, Canary, instead
  of largest first.
- Add **Battle.net - Old launcher versions**. Like the Agent's, it keeps the version Battle.net
  records as installed.

## 1.2.6

- Keep the `Temp`, `Prefetch`, and Windows Update download folders and clean only what they hold.
  Moving the folder itself to the Recycle Bin removed a folder Windows and other programs expect
  to exist.
- Record what a cleanup actually freed. A target that could not be moved counted its whole size
  as freed, so failed runs inflated the totals in **History**. Runs from earlier versions keep
  the size they recorded.
- Keep checkbox and preset changes when you rescan or reset the selection within 30 seconds of
  making them.
- Stop cleaning data that is not a cache:
  - **Misc - Leftover caches** no longer takes the whole `.cache` folder in your profile, which
    holds sign-in tokens such as Hugging Face's.
  - **Playwright** cleans the downloaded browsers and keeps the saved browser profiles.
  - **GOG Galaxy** keeps the launcher's cookies and saved sign-in, and **Spotify** keeps songs
    saved for offline listening.
  - The **Maven**, **uv**, **Bun**, and **Windows - Delivery optimization cache** targets are
    removed. Maven's repository holds artifacts you built yourself, the uv and Bun caches can back
    installed environments, and the Delivery Optimization path was out of date.
  - The **Discord** and **osu! (lazer)** old-version targets are removed. Both apps already remove
    their own old versions.
- **Battle.net - Old agent versions** keeps the version Battle.net records as installed, and checks
  again right before cleaning.
- Find caches the old paths missed: EA, Steam, and Opera caches in newer layouts, Slack from the
  Microsoft Store, the WhatsApp desktop app, Rockstar launcher logs in Documents, Blender's cache,
  deeper PowerToys logs, and AMD's DX12 shader cache.
- Add **NuGet - HTTP cache** and **Windows - Direct3D shader cache**, both unchecked by default.
  The Direct3D cache was part of **Misc - Leftover caches**.
- **NuGet - Package cache** is unchecked by default, because projects build from it and clearing it
  means a restore. **Unity - GI cache** takes only the GI cache folder.
- Rename **pnpm - Store cache** to **Metadata cache** and **Cypress - Downloaded browsers** to
  **Downloaded Cypress versions**, which is what they hold.
- Write log lines to the console when today's log file cannot be opened, instead of dropping them.
- Opening a second copy of the app while the first has an update waiting no longer deletes the
  first copy's download.

## 1.2.5

- **Check for updates** in **About** checks GitHub right away as it opens the update window. It
  only opened the window.
- Cap each day's log file at 10 MiB, so a runaway log cannot fill the disk.
- Get the self-updater, the update window, logging, and the folder lookups from [handsomefox/app-internals](https://github.com/handsomefox/app-internals),
  which all five apps share, instead of a copy inside this app.

## 1.2.4

- Get the self-updater from its own repository, [handsomefox/app-update](https://github.com/handsomefox/app-update), instead of a copy
  inside this app. Checking for, downloading, and installing an update work as before.

## 1.2.3

- Keep presets whose names differ only in the case of a non-ASCII letter, such as "Über" and
  "über". The second one was lost at the next start.
- End the scan or cleanup with a message when the background worker stops unexpectedly. The
  window waited forever.
- Check empty folders against the safe locations during the scan, like every other item. They are
  still checked again before removal.
- Refuse an app update whose checksum on GitHub cannot be read, instead of skipping that check.
- Retry putting the current version back when installing an app update fails.

## 1.2.2

- New installs check for a new version each time the app starts. Existing installs keep their
  setting. To change it, open **About** > **App updates**.
- Keep a found update visible after a restart. With a check period of a day or longer, a restart
  hid the new version until the next check was due.
- Remove the line in **App updates** that said no check had run, which appeared beside the time
  of the last check.

## 1.2.1

- Give Windows Cleaner a new icon.
- Update eframe and egui_extras to 0.37. The first launch after this update opens at the default
  window size and position. Later launches remember the window again.
- Update flate2 to 1.1.10, serde_json to 1.0.154, sha2 to 0.11.1, and zip to 9.2.0.

## 1.2.0

- Windows Cleaner can now update itself from its GitHub releases. It checks once a day, and when
  a new version is out, a chip in the header names it. Click the chip to read what changed since
  your version, then download the update, skip that version, or leave it for later. The download
  must match its published SHA-256 checksums before it replaces the app, and a restart finishes
  the install. A restart waits for a running scan or cleanup. To check more or less often, or
  never, open **About** > **App updates**.
- Getting 1.2.0 itself is still a manual download. Later versions install from inside the app.

## 1.1.4

- Replace the yanked clipboard-win 5.4.1 with 5.4.3.

## 1.1.3

- Update jiff to 0.2.38 and embed-resource to 3.0.12.

## 1.1.2

- Update thiserror to 2.0.21, and replace the yanked yoke-derive 0.8.3 with 0.8.4.

## 1.1.1

- Fix the **Project page** link in About, which did not open.

## 1.1.0

- Update egui to 0.36.2, along with smaller dependencies.
- Build with Rust 1.98.1.

## 1.0.14

- Add presets. **Presets** in the toolbar saves the targets you have checked under a name, and
  picking that name checks them again. Manage presets renames, applies, and deletes them, and
  marks one **Use on start**, which is applied at every launch in place of your last selection.
  A preset stores individual targets, not categories, so it survives a catalog that regroups
  its items, and it changes only when you say so. You can keep 20.
- Apply a preset through the same rules a remembered selection uses: a target with nothing to
  clean is not checked, and empty-folder removal stays opt-in per run.

## 1.0.13

- Keep remembering a target through a run where it is empty or its app is gone. 1.0.12 dropped
  it from the saved selection instead, so a target you cleaned in one run came back unchecked
  two runs later, even once it had data again. An empty target is still never checked for you,
  because there is nothing to free. The choice now waits for the run that has something to
  offer.
- Store the selection a cleanup ran with. It was only saved from the selection screen, so
  cleaning and then closing from the results kept whatever the app had loaded at startup.
- Check the catalog's own picks again on a first launch. 1.0.12 applied an empty remembered
  selection over them, so a new install opened with nothing selected.
- Rename **Forget saved selection** in Settings to **Reset selection to defaults**, and make it
  scan again so the catalog's picks come back on the spot. It used to drop the stored selection
  and nothing else, and the next automatic save wrote the same selection straight back.
- Say 123 cleanup targets in the README. The catalog grew to that in 1.0.11 and the count
  stayed at 82.

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
