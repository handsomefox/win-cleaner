//! The built-in catalog of cleanup targets. All paths derive from the
//! injected [`Roots`] so no usernames are hardcoded and tests can point
//! everything at temp directories. Items whose roots are unavailable are
//! simply omitted.

use std::path::{Path, PathBuf};

use cleaner_core::{Item, Registry, Roots};

/// Builder shorthand.
fn item(app: &str, label: &str, default_on: bool) -> Item {
    Item::new(app, label, default_on)
}

/// The standard per-profile Chromium cache subdirectories rooted at `base`
/// (covers Chrome, Edge, Brave, Vivaldi, Opera, CEF apps).
fn chromium_set(base: &Path) -> Vec<PathBuf> {
    [
        PathBuf::from("Cache"),
        PathBuf::from("Code Cache"),
        PathBuf::from("DawnGraphiteCache"),
        PathBuf::from("DawnWebGPUCache"),
        PathBuf::from("GPUCache"),
        PathBuf::from("GrShaderCache"),
        PathBuf::from("ShaderCache"),
        PathBuf::from("Service Worker").join("CacheStorage"),
        PathBuf::from("Service Worker").join("ScriptCache"),
    ]
    .into_iter()
    .map(|sub| base.join(sub))
    .collect()
}

/// Per-profile glob patterns for a Chromium `User Data` directory, covering
/// Cache and related subdirs for all profiles.
fn chromium_profile_globs(user_data: &Path) -> Vec<PathBuf> {
    [
        PathBuf::from("Cache"),
        PathBuf::from("Code Cache"),
        PathBuf::from("GPUCache"),
        PathBuf::from("DawnWebGPUCache"),
        PathBuf::from("DawnGraphiteCache"),
        PathBuf::from("Service Worker").join("CacheStorage"),
        PathBuf::from("Service Worker").join("ScriptCache"),
    ]
    .into_iter()
    .map(|sub| user_data.join("*").join(sub))
    .collect()
}

/// A Chromium-family browser with shared user-data caches and per-profile caches.
fn chromium_browser(app: &str, user_data: &Path) -> Item {
    chromium_channel(app, "Profile cache", user_data)
}

/// One release channel of a Chromium-family browser. Each channel keeps its
/// own `User Data`, so each is its own target under the browser's entry.
fn chromium_channel(app: &str, label: &str, user_data: &Path) -> Item {
    item(app, label, true)
        .paths([
            user_data.join("extensions_crx_cache"),
            user_data.join("component_crx_cache"),
            user_data.join("GrShaderCache"),
            user_data.join("ShaderCache"),
        ])
        .globs(chromium_profile_globs(user_data))
}

/// The typical Electron app cache directories rooted at `base`.
fn electron_set(base: &Path) -> Vec<PathBuf> {
    vec![
        base.join("Cache"),
        base.join("Code Cache"),
        base.join("GPUCache"),
        base.join("Service Worker").join("CacheStorage"),
    ]
}

/// Cache directories shared by VS Code and editors derived from it. Logs live
/// beside them in `logs`, and are a separate item.
fn vscode_cache_set(base: &Path) -> Vec<PathBuf> {
    chromium_set(base)
        .into_iter()
        .chain([base.join("CachedData"), base.join("CachedExtensionVSIXs")])
        .collect()
}

/// Builds the built-in registry of cleanup targets. Returns an empty registry
/// when the required roots are absent (i.e. on non-Windows platforms);
/// individually gated items (Program Files, `SystemRoot`) are omitted when
/// their root is missing.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "a flat data table reads best unbroken"
)]
pub fn build_registry(roots: &Roots) -> Registry {
    let (Some(local), Some(roaming), Some(program_data), Some(profile)) = (
        roots.local_app_data.as_deref(),
        roots.roaming_app_data.as_deref(),
        roots.program_data.as_deref(),
        roots.user_profile.as_deref(),
    ) else {
        return Registry::default();
    };
    // Always Some beside a profile root, which the destructuring just required.
    let Some(local_low) = roots.local_low() else {
        return Registry::default();
    };
    // Slack from the Microsoft Store keeps the same files inside its package.
    let slack_store = local
        .join("Packages")
        .join("com.tinyspeck.slackdesktop_8yrtsj140pw4g")
        .join("LocalCache")
        .join("Roaming")
        .join("Slack");
    // Newer Opera builds keep the profile's caches one level down, in
    // `Default`, in both the local and the roaming folder.
    let opera = |name: &str| -> Vec<PathBuf> {
        let local_root = local.join("Opera Software").join(name);
        let roaming_root = roaming.join("Opera Software").join(name);
        [
            local_root.clone(),
            local_root.join("Default"),
            roaming_root.join("Default"),
        ]
        .iter()
        .flat_map(|base| chromium_set(base))
        .collect()
    };

    // One item holds one kind of data, so cache, logs, crash reports, and
    // downloaded updates are separate and separately selectable. A label is
    // the short sentence-case noun phrase naming that kind: "Cache", "Logs",
    // "Crash reports". It never repeats the app name and never lists the
    // paths, which the details button in the UI shows.
    let mut items = vec![
        // One entry per browser, with a target per release channel in the
        // order Stable, Beta, Dev, Canary. Canary installs as "SxS".
        chromium_browser(
            "Chrome",
            &local.join("Google").join("Chrome").join("User Data"),
        ),
        chromium_channel(
            "Chrome",
            "Profile cache (Beta)",
            &local.join("Google").join("Chrome Beta").join("User Data"),
        ),
        chromium_channel(
            "Chrome",
            "Profile cache (Dev)",
            &local.join("Google").join("Chrome Dev").join("User Data"),
        ),
        chromium_channel(
            "Chrome",
            "Profile cache (Canary)",
            &local.join("Google").join("Chrome SxS").join("User Data"),
        ),
        chromium_browser(
            "Chrome for Testing",
            &local
                .join("Google")
                .join("Chrome for Testing")
                .join("User Data"),
        ),
        chromium_browser(
            "Edge",
            &local.join("Microsoft").join("Edge").join("User Data"),
        ),
        chromium_channel(
            "Edge",
            "Profile cache (Beta)",
            &local.join("Microsoft").join("Edge Beta").join("User Data"),
        ),
        chromium_channel(
            "Edge",
            "Profile cache (Dev)",
            &local.join("Microsoft").join("Edge Dev").join("User Data"),
        ),
        chromium_channel(
            "Edge",
            "Profile cache (Canary)",
            &local.join("Microsoft").join("Edge SxS").join("User Data"),
        ),
        item("Firefox", "Crash reports", true).paths([roaming
            .join("Mozilla")
            .join("Firefox")
            .join("Crash Reports")]),
        item("Firefox", "Cache", true).globs([
            local
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles")
                .join("*")
                .join("cache2"),
            local
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles")
                .join("*")
                .join("startupCache"),
            local
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles")
                .join("*")
                .join("jumpListCache"),
            local
                .join("Packages")
                .join("Mozilla.Firefox_*")
                .join("LocalCache")
                .join("Local")
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles")
                .join("*")
                .join("cache2"),
            local
                .join("Packages")
                .join("Mozilla.Firefox_*")
                .join("LocalCache")
                .join("Local")
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles")
                .join("*")
                .join("startupCache"),
        ]),
        item("Brave", "Profile cache", true)
            .paths([
                local
                    .join("BraveSoftware")
                    .join("Brave-Browser")
                    .join("User Data")
                    .join("GrShaderCache"),
                local
                    .join("BraveSoftware")
                    .join("Brave-Browser")
                    .join("User Data")
                    .join("ShaderCache"),
            ])
            .globs(chromium_profile_globs(
                &local
                    .join("BraveSoftware")
                    .join("Brave-Browser")
                    .join("User Data"),
            )),
        item("Opera", "Cache", true).paths(opera("Opera Stable")),
        item("Vivaldi", "Profile cache", true).globs(chromium_profile_globs(
            &local.join("Vivaldi").join("User Data"),
        )),
        chromium_browser("Chromium", &local.join("Chromium").join("User Data")),
        item("Opera GX", "Cache", true)
            .paths(opera("Opera GX Stable").into_iter().chain(chromium_set(
                &roaming.join("Opera Software").join("Opera GX Stable"),
            )))
            .globs(
                chromium_set(Path::new("*"))
                    .into_iter()
                    .map(|sub| {
                        local
                            .join("Opera Software")
                            .join("Opera GX Stable")
                            .join("_side_profiles")
                            .join(sub)
                    })
                    .chain(chromium_set(Path::new("*")).into_iter().map(|sub| {
                        roaming
                            .join("Opera Software")
                            .join("Opera GX Stable")
                            .join("_side_profiles")
                            .join(sub)
                    })),
            ),
        item("Thunderbird", "Crash reports", true)
            .paths([roaming.join("Thunderbird").join("Crash Reports")]),
        item("Thunderbird", "Cache", true).globs([
            local
                .join("Thunderbird")
                .join("Profiles")
                .join("*")
                .join("cache2"),
            local
                .join("Thunderbird")
                .join("Profiles")
                .join("*")
                .join("startupCache"),
            local
                .join("Packages")
                .join("MozillaThunderbird.MZLA_*")
                .join("LocalCache")
                .join("Local")
                .join("Thunderbird")
                .join("Profiles")
                .join("*")
                .join("cache2"),
            local
                .join("Packages")
                .join("MozillaThunderbird.MZLA_*")
                .join("LocalCache")
                .join("Local")
                .join("Thunderbird")
                .join("Profiles")
                .join("*")
                .join("startupCache"),
        ]),
        item("Discord", "Cache", true).paths(chromium_set(&roaming.join("discord"))),
        item("Discord", "Logs", true).paths([roaming.join("discord").join("logs")]),
        item("Discord", "Update downloads", true).paths([local.join("Discord").join("download")]),
        item("Slack", "Cache", true).paths(
            [roaming.join("Slack"), slack_store.clone()]
                .iter()
                .flat_map(|base| electron_set(base)),
        ),
        item("Slack", "Logs", true)
            .paths([roaming.join("Slack").join("logs"), slack_store.join("logs")]),
        item("Teams (classic)", "Cache", true).paths([
            roaming.join("Microsoft").join("Teams").join("Cache"),
            roaming.join("Microsoft").join("Teams").join("Code Cache"),
            roaming.join("Microsoft").join("Teams").join("GPUCache"),
            roaming
                .join("Microsoft")
                .join("Teams")
                .join("Service Worker")
                .join("CacheStorage"),
        ]),
        item("Teams (classic)", "Logs", true)
            .paths([roaming.join("Microsoft").join("Teams").join("logs")]),
        item("Teams (new)", "Local cache", true).paths([local
            .join("Packages")
            .join("MSTeams_8wekyb3d8bbwe")
            .join("LocalCache")
            .join("Microsoft")
            .join("MSTeams")]),
        item("Zoom", "Cache", true).paths([roaming.join("Zoom").join("data").join("Cache")]),
        item("Zoom", "Logs", true).paths([roaming.join("Zoom").join("logs")]),
        item("Telegram", "Temp files", true)
            .paths([roaming.join("Telegram Desktop").join("tdata").join("temp")]),
        item("Telegram", "Crash dumps", true)
            .paths([roaming.join("Telegram Desktop").join("tdata").join("dumps")]),
        item("Telegram", "Cache", true).globs([
            roaming
                .join("Telegram Desktop")
                .join("tdata")
                .join("user_data*")
                .join("cache"),
            roaming
                .join("Telegram Desktop")
                .join("tdata")
                .join("user_data*")
                .join("media_cache"),
        ]),
        // The Electron desktop app. The Store app that replaced it keeps its
        // data in a package folder this does not touch.
        item("WhatsApp", "Cache", true).paths(electron_set(&roaming.join("WhatsApp"))),
        item("Signal", "Cache", true).paths(electron_set(&roaming.join("Signal"))),
        item("Signal", "Logs", true).paths([roaming.join("Signal").join("logs")]),
        item("Steam", "HTML cache", true).paths(
            chromium_set(&local.join("Steam").join("htmlcache"))
                .into_iter()
                .chain(chromium_set(
                    &local.join("Steam").join("htmlcache").join("Default"),
                )),
        ),
        item("Battle.net", "Browser cache", true).paths(
            chromium_set(
                &local
                    .join("Battle.net")
                    .join("BrowserCaches")
                    .join("common"),
            )
            .into_iter()
            .chain([local.join("Battle.net").join("Cache")]),
        ),
        item("Battle.net", "Logs", true).paths([local.join("Battle.net").join("Logs")]),
        // `.product.db` names the installed Agent build; see `Versioned`.
        item("Battle.net", "Old agent versions", false)
            .versioned(program_data.join("Battle.net").join("Agent"), "Agent."),
        item("Epic Games Launcher", "Web cache", true)
            .paths([local
                .join("EpicGamesLauncher")
                .join("Saved")
                .join("webcache")])
            .globs([local
                .join("EpicGamesLauncher")
                .join("Saved")
                .join("webcache_*")]),
        // `webcache` is a whole browser profile with cookies and local
        // storage, so only its cache folders are cleaned.
        item("GOG Galaxy", "Web cache", true).paths(
            chromium_set(&program_data.join("GOG.com").join("Galaxy").join("webcache"))
                .into_iter()
                .chain(chromium_set(
                    &program_data
                        .join("GOG.com")
                        .join("Galaxy")
                        .join("webcache")
                        .join("Default"),
                )),
        ),
        item("EA/Origin", "Cache", true).paths(
            chromium_set(
                &local
                    .join("Electronic Arts")
                    .join("EA Desktop")
                    .join("CEF")
                    .join("BrowserCache")
                    .join("EADesktop"),
            )
            .into_iter()
            .chain([
                local.join("EADesktop").join("cache"),
                local.join("Link2EA").join("cache"),
                local.join("EALaunchHelper").join("cache"),
            ]),
        ),
        item("EA/Origin", "Logs", true).paths([
            program_data.join("EA Desktop").join("Logs"),
            program_data.join("EA Logs"),
            program_data.join("Origin").join("Logs"),
        ]),
        item("Rockstar Games Launcher", "Cache", true).paths([
            local.join("Rockstar Games").join("Launcher").join("Cache"),
            local
                .join("Rockstar Games")
                .join("Launcher")
                .join("webcache"),
        ]),
        // Rockstar's support pages place the launcher's logs in Documents.
        item("Rockstar Games Launcher", "Logs", true).globs([profile
            .join("Documents")
            .join("Rockstar Games")
            .join("Launcher")
            .join("*.log")]),
        item("Battlefield 2042", "Cache", true).paths([local
            .join("BattlefieldGameData.kin-release.Win32")
            .join("cache")]),
        item("osu! (lazer)", "Cache", true).paths([roaming.join("osu").join("cache")]),
        item("osu! (lazer)", "Logs", true).paths([roaming.join("osu").join("logs")]),
        item("VSCode", "Cache", true).paths(vscode_cache_set(&roaming.join("Code"))),
        item("VSCode", "Logs", true).paths([roaming.join("Code").join("logs")]),
        item("Cursor", "Cache", true).paths(vscode_cache_set(&roaming.join("Cursor"))),
        item("Cursor", "Logs", true).paths([roaming.join("Cursor").join("logs")]),
        item("VSCodium", "Cache", true).paths(vscode_cache_set(&roaming.join("VSCodium"))),
        item("VSCodium", "Logs", true).paths([roaming.join("VSCodium").join("logs")]),
        item("GitHub Desktop", "Cache", true).globs([
            roaming.join("GitHub Desktop").join("*Cache"),
            roaming.join("GitHubDesktop").join("*Cache"),
        ]),
        item("GitHub Desktop", "Logs", true).globs([
            roaming.join("GitHub Desktop").join("logs"),
            roaming.join("GitHubDesktop").join("logs"),
        ]),
        item("Postman", "Cache", true).paths(
            electron_set(&roaming.join("Postman"))
                .into_iter()
                .chain([roaming
                    .join("Postman")
                    .join("Partitions")
                    .join("postman")
                    .join("GPUCache")]),
        ),
        item("Postman", "Logs", true).paths([roaming.join("Postman").join("logs")]),
        item("Obsidian", "Cache", true).paths(electron_set(&roaming.join("obsidian"))),
        item("Obsidian", "Logs", true).globs([roaming.join("obsidian").join("*.log")]),
        item("Android Studio", "Logs", true)
            .globs([local.join("Google").join("AndroidStudio*").join("log")]),
        item("Android Studio", "Cache", false)
            .globs([local.join("Google").join("AndroidStudio*").join("caches")]),
        item("JetBrains", "Cache", true).globs([local.join("JetBrains").join("*").join("caches")]),
        item("JetBrains", "Logs", false).globs([local.join("JetBrains").join("*").join("log")]),
        item("npm", "Package cache", true).paths([local.join("npm-cache")]),
        item("Yarn", "Package cache", true).paths([local.join("Yarn").join("Cache")]),
        item("Go modules", "Download cache", false).paths([profile
            .join("go")
            .join("pkg")
            .join("mod")
            .join("cache")]),
        item("Cargo", "Registry cache", false)
            .paths([profile.join(".cargo").join("registry").join("cache")]),
        item("Cargo", "Git cache", false).paths([profile.join(".cargo").join("git").join("db")]),
        item("Gradle", "Build cache", false).paths([profile.join(".gradle").join("caches")]),
        // Projects build from these packages directly; clearing it means a
        // restore, and sometimes reloading an open solution.
        item("NuGet", "Package cache", false).paths([profile.join(".nuget").join("packages")]),
        item("NuGet", "HTTP cache", false).paths([local.join("NuGet").join("v3-cache")]),
        item("pip", "Download cache", false).paths([local.join("pip").join("Cache")]),
        // Package metadata and dlx downloads. The package store is elsewhere.
        item("pnpm", "Metadata cache", false).paths([local.join("pnpm-cache")]),
        item("Cypress", "Downloaded Cypress versions", false)
            .paths([local.join("Cypress").join("Cache")]),
        // The browser builds only. `daemon` holds saved browser profiles.
        item("Playwright", "Downloaded browsers", false).globs(
            [
                "chromium-*",
                "chromium_headless_shell-*",
                "chromium_tip_of_tree-*",
                "firefox-*",
                "webkit-*",
                "ffmpeg-*",
                "winldd-*",
            ]
            .map(|name| local.join("ms-playwright").join(name)),
        ),
        item("Visual Studio", "Component model cache", false).globs([local
            .join("Microsoft")
            .join("VisualStudio")
            .join("*")
            .join("ComponentModelCache")]),
        item("Unity", "GI cache", false)
            .paths([local_low.join("Unity").join("Caches").join("GiCache")]),
        item("NVIDIA", "App cache", true).paths(chromium_set(
            &local
                .join("NVIDIA Corporation")
                .join("NVIDIA App")
                .join("CefCache"),
        )),
        item("NVIDIA", "Update downloads", true).paths([
            program_data
                .join("NVIDIA Corporation")
                .join("NVIDIA App")
                .join("UpdateFramework")
                .join("ota-artifacts")
                .join("grd"),
            program_data
                .join("NVIDIA Corporation")
                .join("NVIDIA App")
                .join("UpdateFramework")
                .join("ota-artifacts")
                .join("nvapp"),
        ]),
        item("NVIDIA", "Logs", true).paths([program_data
            .join("NVIDIA Corporation")
            .join("NVIDIA App")
            .join("Logs")]),
        item("NVIDIA", "Shader cache", false).paths([
            local.join("NVIDIA").join("DXCache"),
            local.join("NVIDIA").join("GLCache"),
            // The driver writes a second, often larger shader cache here.
            local_low.join("NVIDIA").join("DXCache"),
        ]),
        item("DLSS Updater", "Cache", true).paths([local.join("DLSS Updater").join("cache")]),
        item("DLSS Updater", "Logs", true).paths([local.join("DLSS Updater").join("logs")]),
        item("AMD", "Shader cache", false).paths([
            local.join("AMD").join("DxCache"),
            local.join("AMD").join("DxcCache"),
            local.join("AMD").join("VkCache"),
            local.join("AMD").join("OglCache"),
        ]),
        item("Razer Synapse", "Cache", true).paths(chromium_set(
            &local
                .join("Razer")
                .join("RazerAppEngine")
                .join("User Data")
                .join("Default"),
        )),
        // `Storage` is the default location for downloaded songs, so it stays.
        item("Spotify", "Streaming cache", true)
            .paths([local.join("Spotify").join("Data")])
            .globs(
                [local
                    .join("Packages")
                    .join("SpotifyAB.SpotifyMusic_*")
                    .join("LocalCache")
                    .join("Spotify")
                    .join("Data")]
                .into_iter()
                .chain(chromium_set(
                    &local
                        .join("Packages")
                        .join("SpotifyAB.SpotifyMusic_*")
                        .join("LocalCache")
                        .join("Spotify"),
                )),
            ),
        item("VLC", "Crash dumps", true).paths([roaming.join("vlc").join("crashdump")]),
        item("OBS Studio", "Cache", true).paths([roaming
            .join("obs-studio")
            .join("plugin_config")
            .join("obs-browser")
            .join("cache")]),
        item("OBS Studio", "Logs", true).paths([roaming.join("obs-studio").join("logs")]),
        item("OBS Studio", "Crash reports", true)
            .paths([roaming.join("obs-studio").join("crashes")]),
        item("OBS Studio", "Update downloads", true)
            .paths([roaming.join("obs-studio").join("updates")]),
        item("Adobe", "Media cache", false).paths([roaming
            .join("Adobe")
            .join("Common")
            .join("Media Cache Files")]),
        item("Blender", "Cache", true).paths([local
            .join("Blender Foundation")
            .join("Blender")
            .join("Cache")]),
        item("Figma", "Desktop cache", true).paths([roaming.join("Figma").join("Desktop")]),
        item("Notion", "Cache", true).paths(electron_set(&roaming.join("Notion"))),
        item("Vortex", "Cache", true).paths(chromium_set(&roaming.join("Vortex"))),
        item("Vortex", "Temp files", true).paths([roaming.join("Vortex").join("temp")]),
        item("qBittorrent", "Cache", true).paths([local.join("qBittorrent").join("cache")]),
        item("qBittorrent", "Logs", true).paths([local.join("qBittorrent").join("Logs")]),
        item("Cloudflare WARP", "Update downloads", true)
            .paths([local.join("Cloudflare").join("updates")]),
        item("Cloudflare WARP", "Logs", true).globs([local.join("Cloudflare").join("*.log")]),
        item("PowerToys", "Logs", true).globs([
            local.join("Microsoft").join("PowerToys").join("*.log"),
            local
                .join("Microsoft")
                .join("PowerToys")
                .join("*")
                .join("Logs"),
            // Modules with their own tools nest deeper, as
            // `FancyZones\Editor\Logs`.
            local
                .join("Microsoft")
                .join("PowerToys")
                .join("*")
                .join("*")
                .join("Logs"),
        ]),
        item("Riot Client", "Logs", true)
            .paths([local.join("Riot Games").join("Riot Client").join("Logs")]),
        item("Riot Client", "Crash reports", true).globs([local
            .join("Riot Games")
            .join("Riot Client")
            .join("Crashes")
            .join("Riot Client *")]),
        item("Minecraft", "Logs", true).paths([
            roaming.join(".minecraft").join("logs"),
            roaming.join(".minecraft").join("debug"),
        ]),
        item("Minecraft", "Crash reports", true)
            .paths([roaming.join(".minecraft").join("crash-reports")]),
        item("Roblox", "Logs", true)
            .paths([local.join("Roblox").join("logs")])
            .globs([local
                .join("Packages")
                .join("ROBLOXCORPORATION.ROBLOX_*")
                .join("LocalState")
                .join("logs")]),
        item("Roblox", "Download cache", false).paths([
            local.join("Roblox").join("Downloads"),
            program_data.join("Roblox").join("Downloads"),
        ]),
        // Programs expect `Temp` to exist, so it stays and what it holds goes.
        item("Windows", "Temp files", true).contents([local.join("Temp")]),
        item("Windows", "Internet cache", true)
            .paths([local.join("Microsoft").join("Windows").join("INetCache")]),
        item("Windows", "Thumbnail cache", true).globs([local
            .join("Microsoft")
            .join("Windows")
            .join("Explorer")
            .join("thumbcache*.db")]),
        item("Windows", "Icon cache", true).globs([local
            .join("Microsoft")
            .join("Windows")
            .join("Explorer")
            .join("iconcache*")]),
        item("Windows Error Reporting", "Report archives", true).paths([
            local
                .join("Microsoft")
                .join("Windows")
                .join("WER")
                .join("ReportArchive"),
            local
                .join("Microsoft")
                .join("Windows")
                .join("WER")
                .join("ReportQueue"),
        ]),
        item("Crash dumps", "Dump files", true).paths([local.join("CrashDumps")]),
        // The launcher installed from the store keeps its cache under the
        // profile instead of its install folder.
        item("Ubisoft Connect", "Cache", true)
            .paths([local.join("Ubisoft Game Launcher").join("cache")]),
        item("Ubisoft Connect", "Logs", true)
            .paths([local.join("Ubisoft Game Launcher").join("logs")]),
        item("Windows", "Direct3D shader cache", false).paths([local.join("D3DSCache")]),
        item("Misc", "Leftover caches", true).paths([
            local.join("cache"),
            local.join("vlc").join("cache"),
            profile.join("ansel"),
        ]),
    ];

    if let Some(x86) = roots.program_files_x86.as_deref() {
        // The launcher's own builds, beside the `.product.db` naming the
        // installed one; see `Versioned`.
        items.push(
            item("Battle.net", "Old launcher versions", false)
                .versioned(x86.join("Battle.net"), "Battle.net."),
        );
        items.push(
            item("Ubisoft Connect", "Install folder cache", true).paths([x86
                .join("Ubisoft")
                .join("Ubisoft Game Launcher")
                .join("cache")]),
        );
        items.push(
            item("Ubisoft Connect", "Install folder logs", true).paths([x86
                .join("Ubisoft")
                .join("Ubisoft Game Launcher")
                .join("logs")]),
        );
    }
    if let Some(system_root) = roots.system_root.as_deref() {
        // Windows recreates neither folder on demand, so both stay and only
        // what they hold goes.
        items.push(
            item("Windows", "Update downloads", true)
                .contents([system_root.join("SoftwareDistribution").join("Download")]),
        );
        items
            .push(item("Windows", "Prefetch data", false).contents([system_root.join("Prefetch")]));
    }

    let system: Vec<&Path> = [
        &roots.program_files,
        &roots.program_files_x86,
        &roots.program_w6432,
        &roots.system_root,
        &roots.windir,
    ]
    .into_iter()
    .filter_map(|root| root.as_deref())
    .collect();
    for item in &mut items {
        item.admin = needs_admin(item, &system);
        if item.processes.is_empty() {
            let names = processes_of(&item_key(item));
            *item = std::mem::take(item).processes(names.iter().copied());
        }
    }

    Registry { items }
}

fn item_key(item: &Item) -> (String, String) {
    (item.app.clone(), item.label.clone())
}

/// Whether any path of `item` lies under Program Files or Windows, where
/// moving files needs administrator rights.
fn needs_admin(item: &Item, system: &[&Path]) -> bool {
    item.paths
        .iter()
        .chain(&item.globs)
        .chain(&item.contents)
        .chain(item.versioned.iter().map(|rule| &rule.dir))
        .any(|path| system.iter().any(|root| path.starts_with(root)))
}

/// The programs whose running keeps a target's files in use. Old program
/// versions have none: their version rule already protects the build in
/// use, and Battle.net's Agent runs whenever the launcher does. Shared
/// Windows folders and command-line tools have none either.
fn processes_of((app, label): &(String, String)) -> &'static [&'static str] {
    if label.starts_with("Old ") {
        return &[];
    }
    match app.as_str() {
        "Chrome" | "Chromium" | "Chrome for Testing" => &["chrome.exe"],
        "Edge" => &["msedge.exe"],
        "Firefox" => &["firefox.exe"],
        "Brave" => &["brave.exe"],
        "Opera" | "Opera GX" => &["opera.exe"],
        "Vivaldi" => &["vivaldi.exe"],
        "Thunderbird" => &["thunderbird.exe"],
        "Discord" => &["Discord.exe"],
        "Slack" => &["slack.exe"],
        "Teams (classic)" => &["Teams.exe"],
        "Teams (new)" => &["ms-teams.exe"],
        "Zoom" => &["Zoom.exe"],
        "Telegram" => &["Telegram.exe"],
        "WhatsApp" => &["WhatsApp.exe"],
        "Signal" => &["Signal.exe"],
        "Steam" => &["steam.exe", "steamwebhelper.exe"],
        "Battle.net" => &["Battle.net.exe"],
        "Epic Games Launcher" => &["EpicGamesLauncher.exe"],
        "GOG Galaxy" => &["GalaxyClient.exe"],
        "EA/Origin" => &["EADesktop.exe", "Origin.exe"],
        "Rockstar Games Launcher" => &["RockstarService.exe"],
        "Battlefield 2042" => &["BF2042.exe"],
        "osu! (lazer)" => &["osu!.exe"],
        "VSCode" => &["Code.exe"],
        "Cursor" => &["Cursor.exe"],
        "VSCodium" => &["VSCodium.exe"],
        "GitHub Desktop" => &["GitHubDesktop.exe"],
        "Postman" => &["Postman.exe"],
        "Obsidian" => &["Obsidian.exe"],
        "Android Studio" => &["studio64.exe"],
        "Visual Studio" => &["devenv.exe"],
        "Unity" => &["Unity.exe"],
        "NVIDIA" => &["NVIDIA app.exe"],
        "Razer Synapse" => &["RazerAppEngine.exe"],
        "Spotify" => &["Spotify.exe"],
        "VLC" => &["vlc.exe"],
        "OBS Studio" => &["obs64.exe"],
        "Blender" => &["blender.exe"],
        "Figma" => &["Figma.exe"],
        "Notion" => &["Notion.exe"],
        "Vortex" => &["Vortex.exe"],
        "qBittorrent" => &["qbittorrent.exe"],
        "Cloudflare WARP" => &["Cloudflare WARP.exe"],
        "PowerToys" => &["PowerToys.exe"],
        "Riot Client" => &["RiotClientServices.exe"],
        "Roblox" => &["RobloxPlayerBeta.exe", "RobloxStudioBeta.exe"],
        "Ubisoft Connect" => &["upc.exe"],
        "DLSS Updater" => &["dlss-updater.exe"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::{Roots, build_plan, is_safe_path};
    use std::collections::HashSet;
    use std::fs::{create_dir_all, write};
    use std::path::PathBuf;

    fn test_roots(base: &Path) -> Roots {
        Roots {
            local_app_data: Some(base.join("Local")),
            roaming_app_data: Some(base.join("Roaming")),
            program_data: Some(base.join("ProgramData")),
            user_profile: Some(base.join("Profile")),
            program_files_x86: Some(base.join("ProgramFilesX86")),
            ..Roots::default()
        }
    }

    fn write_file(path: &Path, len: usize) {
        create_dir_all(path.parent().unwrap()).unwrap();
        write(path, vec![0u8; len]).unwrap();
    }

    #[test]
    fn empty_registry_without_required_roots() {
        assert!(build_registry(&Roots::default()).items.is_empty());
        let partial = Roots {
            local_app_data: Some(PathBuf::from("/tmp/x")),
            ..Roots::default()
        };
        assert!(build_registry(&partial).items.is_empty());
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one test pins the whole registry shape"
    )]
    fn full_registry_shape() {
        let mut roots = test_roots(Path::new("/base"));
        roots.system_root = Some(PathBuf::from("/base/Windows"));
        let registry = build_registry(&roots);
        assert_eq!(registry.items.len(), 123);

        let chrome = registry
            .items
            .iter()
            .find(|item| item.app == "Chrome")
            .unwrap();
        assert!(chrome.default_on);
        assert_eq!(chrome.paths.len(), 4);
        assert_eq!(chrome.globs.len(), 7);

        // Telegram splits into cache, temp files, and crash dumps.
        let telegram: Vec<&str> = registry
            .items
            .iter()
            .filter(|item| item.app == "Telegram")
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(telegram, vec!["Temp files", "Crash dumps", "Cache"]);
        let telegram_cache = registry
            .items
            .iter()
            .find(|item| item.app == "Telegram" && item.label == "Cache")
            .unwrap();
        assert!(
            telegram_cache.globs[0]
                .to_string_lossy()
                .contains("user_data*")
        );

        // Two JetBrains and two Cargo items with distinct labels.
        assert_eq!(
            registry
                .items
                .iter()
                .filter(|i| i.app == "JetBrains")
                .count(),
            2
        );
        assert_eq!(
            registry.items.iter().filter(|i| i.app == "Cargo").count(),
            2
        );

        let prefetch = registry
            .items
            .iter()
            .find(|i| i.app == "Windows" && i.label == "Prefetch data")
            .unwrap();
        assert!(!prefetch.default_on);
        assert_eq!(
            prefetch.contents.len(),
            1,
            "the Prefetch folder itself stays"
        );

        // Ubisoft keeps a cache under the profile and another in its install
        // folder, each with its own logs.
        let ubisoft: Vec<(&str, usize)> = registry
            .items
            .iter()
            .filter(|item| item.app == "Ubisoft Connect")
            .map(|item| (item.label.as_str(), item.paths.len()))
            .collect();
        assert_eq!(
            ubisoft,
            vec![
                ("Cache", 1),
                ("Logs", 1),
                ("Install folder cache", 1),
                ("Install folder logs", 1)
            ]
        );

        for (app, label, root) in [
            (
                "Chrome",
                "Profile cache",
                "/base/Local/Google/Chrome/User Data",
            ),
            (
                "Chrome",
                "Profile cache (Beta)",
                "/base/Local/Google/Chrome Beta/User Data",
            ),
            (
                "Chrome",
                "Profile cache (Dev)",
                "/base/Local/Google/Chrome Dev/User Data",
            ),
            (
                "Chrome",
                "Profile cache (Canary)",
                "/base/Local/Google/Chrome SxS/User Data",
            ),
            (
                "Chrome for Testing",
                "Profile cache",
                "/base/Local/Google/Chrome for Testing/User Data",
            ),
            (
                "Edge",
                "Profile cache",
                "/base/Local/Microsoft/Edge/User Data",
            ),
            (
                "Edge",
                "Profile cache (Canary)",
                "/base/Local/Microsoft/Edge SxS/User Data",
            ),
            (
                "Chromium",
                "Profile cache",
                "/base/Local/Chromium/User Data",
            ),
        ] {
            let browser = registry
                .items
                .iter()
                .find(|item| item.app == app && item.label == label)
                .unwrap();
            assert!(browser.default_on, "{app}");
            assert_eq!(browser.paths.len(), 4, "{app}");
            assert_eq!(browser.globs.len(), 7, "{app}");
            assert!(
                browser.paths.iter().all(|path| path.starts_with(root)),
                "{app}"
            );
            assert!(
                browser.globs.iter().all(|path| path.starts_with(root)),
                "{app}"
            );
        }

        let firefox = registry
            .items
            .iter()
            .find(|i| i.app == "Firefox" && i.label == "Cache")
            .unwrap();
        let firefox_globs: Vec<_> = firefox
            .globs
            .iter()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert!(
            firefox_globs
                .iter()
                .all(|path| !path.contains("default-release"))
        );
        assert!(firefox_globs.iter().any(|path| {
            path.contains(
                "Packages/Mozilla.Firefox_*/LocalCache/Local/Mozilla/Firefox/Profiles/*/cache2",
            )
        }));
        let thunderbird = registry
            .items
            .iter()
            .find(|i| i.app == "Thunderbird" && i.label == "Cache")
            .unwrap();
        assert!(thunderbird.globs.iter().any(|path| {
            path.to_string_lossy().replace('\\', "/").contains(
                "Packages/MozillaThunderbird.MZLA_*/LocalCache/Local/Thunderbird/Profiles/*/cache2",
            )
        }));

        let all_paths: Vec<_> = registry
            .items
            .iter()
            .flat_map(|item| item.paths.iter().chain(&item.globs))
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect();
        for unsupported in [
            "LocalCache/Roaming/Thunderbird",
            ".minecraft/webcache2",
            "PowerToys/*.etl",
            "ProgramData/Roblox/Logs",
        ] {
            assert!(
                all_paths.iter().all(|path| !path.contains(unsupported)),
                "unsupported catalog path: {unsupported}"
            );
        }

        for app in ["Android Studio", "Cypress", "Playwright"] {
            let opt_in = registry
                .items
                .iter()
                .find(|item| item.app == app && !item.default_on)
                .unwrap_or_else(|| panic!("missing opt-in item for {app}"));
            assert!(!opt_in.default_on);
        }
    }

    #[test]
    fn every_catalog_item_is_unique_nonempty_and_under_a_guard_root() {
        let dir = tempfile::tempdir().unwrap();
        let mut roots = test_roots(dir.path());
        roots.system_root = Some(dir.path().join("Windows"));
        let registry = build_registry(&roots);
        let guard_roots = roots.guard_roots();
        let mut keys = HashSet::new();

        for item in &registry.items {
            assert!(!item.app.trim().is_empty());
            assert!(!item.label.trim().is_empty());
            assert!(
                !item.paths.is_empty()
                    || !item.globs.is_empty()
                    || !item.contents.is_empty()
                    || !item.versioned.is_empty(),
                "{} - {} has no cleanup paths",
                item.app,
                item.label
            );
            assert!(
                keys.insert((item.app.as_str(), item.label.as_str())),
                "duplicate catalog item: {} - {}",
                item.app,
                item.label
            );
            for path in item
                .paths
                .iter()
                .chain(&item.globs)
                .chain(&item.contents)
                .chain(item.versioned.iter().map(|versioned| &versioned.dir))
            {
                assert!(
                    is_safe_path(path, &guard_roots),
                    "unsafe catalog path for {} - {}: {}",
                    item.app,
                    item.label,
                    path.display()
                );
            }
        }
    }

    #[test]
    fn gated_items_are_omitted_when_roots_are_missing() {
        // No SystemRoot and no Program Files (x86): those items don't exist.
        let mut roots = test_roots(Path::new("/base"));
        roots.program_files_x86 = None;
        let registry = build_registry(&roots);
        assert_eq!(registry.items.len(), 118);
        // The profile cache stays; only the install-folder item is gated.
        assert!(
            !registry
                .items
                .iter()
                .any(|item| item.label == "Install folder cache")
        );
        assert!(
            !registry
                .items
                .iter()
                .any(|item| item.label == "Prefetch data")
        );
    }

    /// Superseded versions are program files, so they stay opt-in. Only
    /// Blizzard's layout, with a `.product.db` naming the installed build, is
    /// cleaned: Squirrel and Velopack apps remove their own old versions.
    #[test]
    fn version_items_are_opt_in_and_blizzard_only() {
        let registry = build_registry(&test_roots(Path::new("/base")));
        let versioned: Vec<&Item> = registry
            .items
            .iter()
            .filter(|item| !item.versioned.is_empty())
            .collect();
        let labels: Vec<(&str, &str)> = versioned
            .iter()
            .map(|item| (item.app.as_str(), item.label.as_str()))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("Battle.net", "Old agent versions"),
                ("Battle.net", "Old launcher versions")
            ]
        );
        assert!(versioned.iter().all(|item| !item.default_on));
        assert_eq!(versioned[0].versioned[0].prefix, "Agent.");
        assert_eq!(versioned[1].versioned[0].prefix, "Battle.net.");
        assert!(
            versioned.iter().all(|item| item.processes.is_empty()),
            "the version rule protects the build in use; the Agent always runs"
        );
    }

    #[test]
    fn browser_channels_share_one_entry_in_release_order() {
        let registry = build_registry(&test_roots(Path::new("/base")));
        for (app, folders) in [
            (
                "Chrome",
                ["Chrome", "Chrome Beta", "Chrome Dev", "Chrome SxS"],
            ),
            ("Edge", ["Edge", "Edge Beta", "Edge Dev", "Edge SxS"]),
        ] {
            let items: Vec<&Item> = registry.items.iter().filter(|i| i.app == app).collect();
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert_eq!(
                labels,
                [
                    "Profile cache",
                    "Profile cache (Beta)",
                    "Profile cache (Dev)",
                    "Profile cache (Canary)"
                ],
                "{app}"
            );
            for (item, folder) in items.iter().zip(folders) {
                assert!(
                    item.paths[0]
                        .to_string_lossy()
                        .contains(&format!("/{folder}/User Data")),
                    "{app} {} reads {folder}",
                    item.label
                );
            }
        }
        for gone in ["Chrome Beta", "Chrome Dev", "Chrome Canary"] {
            assert!(registry.items.iter().all(|item| item.app != gone));
        }
    }

    #[test]
    fn system_targets_need_admin_and_app_targets_name_their_program() {
        let mut roots = test_roots(Path::new("/base"));
        roots.system_root = Some(PathBuf::from("/base/Windows"));
        let registry = build_registry(&roots);
        let find = |app: &str, label: &str| {
            registry
                .items
                .iter()
                .find(|i| i.app == app && i.label == label)
                .unwrap()
        };
        for (app, label) in [
            ("Windows", "Update downloads"),
            ("Windows", "Prefetch data"),
            ("Battle.net", "Old launcher versions"),
            ("Ubisoft Connect", "Install folder cache"),
        ] {
            assert!(find(app, label).admin, "{app} - {label}");
        }
        for (app, label) in [("Windows", "Temp files"), ("Chrome", "Profile cache")] {
            assert!(!find(app, label).admin, "{app} - {label}");
        }
        assert_eq!(
            find("Chrome", "Profile cache (Beta)").processes,
            ["chrome.exe"]
        );
        assert_eq!(
            find("Steam", "HTML cache").processes,
            ["steam.exe", "steamwebhelper.exe"]
        );
        assert!(find("Windows", "Temp files").processes.is_empty());
    }

    /// Folders other programs expect to exist keep the folder and lose only
    /// what it holds.
    #[test]
    fn temp_folders_are_emptied_not_removed() {
        let mut roots = test_roots(Path::new("/base"));
        roots.system_root = Some(PathBuf::from("/base/Windows"));
        let registry = build_registry(&roots);
        for label in ["Temp files", "Update downloads", "Prefetch data"] {
            let item = registry
                .items
                .iter()
                .find(|item| item.app == "Windows" && item.label == label)
                .unwrap();
            assert!(item.paths.is_empty() && item.globs.is_empty(), "{label}");
            assert_eq!(item.contents.len(), 1, "{label}");
        }
    }

    /// Targets the path audit found holding data that is not a cache, or that
    /// other programs rely on, are gone and stay gone.
    #[test]
    fn data_that_is_not_disposable_is_not_listed() {
        let mut roots = test_roots(Path::new("/base"));
        roots.system_root = Some(PathBuf::from("/base/Windows"));
        let registry = build_registry(&roots);
        let every_path: Vec<&PathBuf> = registry
            .items
            .iter()
            .flat_map(|item| item.paths.iter().chain(&item.globs).chain(&item.contents))
            .collect();
        for gone in [
            "/base/Profile/.cache",
            "/base/Profile/.m2/repository",
            "/base/Local/uv/cache",
            "/base/Profile/.bun/install/cache",
            "/base/Local/ms-playwright",
            "/base/Local/Spotify/Storage",
            "/base/ProgramData/GOG.com/Galaxy/webcache",
            "/base/ProgramData/Microsoft/Windows/DeliveryOptimization/Cache",
            "/base/Local/Discord/app-*",
            "/base/Local/osulazer/packages/*.nupkg",
        ] {
            assert!(
                !every_path.contains(&&PathBuf::from(gone)),
                "{gone} must not be a cleanup path"
            );
        }
        let d3d = registry
            .items
            .iter()
            .find(|item| item.label == "Direct3D shader cache")
            .unwrap();
        assert_eq!(d3d.app, "Windows");
        assert!(!d3d.default_on);
    }

    /// End-to-end: the built-in catalog scanned against a fake profile tree.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture verifies all catalog path and default interactions together"
    )]
    fn build_plan_scans_catalog_and_respects_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let local = roots.local_app_data.clone().unwrap();

        // Default-on static and nested profile globs plus an opt-in cache.
        write_file(
            &local.join("Google/Chrome/User Data/Default/Cache/f_0001"),
            1000,
        );
        write_file(&local.join("npm-cache/pkg.tgz"), 500);
        write_file(
            &local.join("Opera Software/Opera GX Stable/_side_profiles/work/Cache/data.bin"),
            200,
        );
        write_file(
            &local.join("Thunderbird/Profiles/profile-a/cache2/entries/cache.bin"),
            300,
        );
        write_file(
            &local.join("Packages/MozillaThunderbird.MZLA_abc/LocalCache/Local/Thunderbird/Profiles/profile-b/cache2/entries/cache.bin"),
            310,
        );
        write_file(
            &local.join("Packages/Mozilla.Firefox_abc/LocalCache/Local/Mozilla/Firefox/Profiles/profile-a/cache2/entries/cache.bin"),
            250,
        );
        write_file(
            &local.join("Microsoft/PowerToys/PowerToys Run/Logs/runner.log"),
            50,
        );
        write_file(
            &local.join("Packages/ROBLOXCORPORATION.ROBLOX_abc/LocalState/logs/player.log"),
            60,
        );
        write_file(
            &local.join("Google/Chrome Beta/User Data/Profile 1/Code Cache/js/index"),
            110,
        );
        write_file(&local.join("uv/cache/archive-v0/package.bin"), 400);
        write_file(&local.join("Cypress/Cache/14.0/Cypress.exe"), 450);

        // Persistent data and unsupported alternates must not contribute to a group.
        write_file(
            &local.join("Packages/MozillaThunderbird.MZLA_abc/LocalCache/Roaming/Thunderbird/Profiles/profile-b/ImapMail/inbox"),
            10_000,
        );
        write_file(
            &roots
                .roaming_app_data
                .as_ref()
                .unwrap()
                .join(".minecraft/webcache2/data"),
            10_000,
        );
        write_file(&local.join("Microsoft/PowerToys/settings.json"), 10_000);
        write_file(
            &roots
                .program_data
                .as_ref()
                .unwrap()
                .join("Roblox/Logs/old.log"),
            10_000,
        );

        let registry = build_registry(&roots);
        let mut updates = 0;
        let plan = build_plan(
            &registry,
            &roots,
            &cleaner_core::Rules::default(),
            &|_| false,
            |_| updates += 1,
        );
        assert_eq!(updates, registry.items.len());

        let chrome = plan
            .groups
            .iter()
            .find(|group| group.app == "Chrome")
            .unwrap();
        assert_eq!(chrome.bytes, 1000);
        assert!(chrome.on, "non-empty default-on group is pre-selected");

        let npm = plan.groups.iter().find(|group| group.app == "npm").unwrap();
        assert_eq!(npm.bytes, 500);
        assert!(npm.on);

        for (app, label, bytes) in [
            ("Chrome", "Profile cache (Beta)", 110),
            ("Firefox", "Cache", 250),
            ("Opera GX", "Cache", 200),
            ("Thunderbird", "Cache", 610),
            ("PowerToys", "Logs", 50),
            ("Roblox", "Logs", 60),
        ] {
            let group = plan
                .groups
                .iter()
                .find(|group| group.app == app && group.label == label)
                .unwrap();
            assert_eq!(group.bytes, bytes, "{app}");
            assert!(group.on, "{app}");
        }

        assert!(
            plan.groups.iter().all(|group| group.app != "uv"),
            "uv's cache backs symlinked environments and is not listed"
        );

        let cypress = plan
            .groups
            .iter()
            .find(|group| group.app == "Cypress")
            .unwrap();
        assert_eq!(cypress.bytes, 450);
        assert!(!cypress.on);

        // Empty default-on groups are never pre-selected.
        let edge = plan
            .groups
            .iter()
            .find(|group| group.app == "Edge")
            .unwrap();
        assert_eq!(edge.bytes, 0);
        assert!(!edge.on);

        // Opt-in groups stay off even with content.
        write_file(&local.join("NVIDIA/DXCache/blob.bin"), 100);
        let plan = build_plan(
            &registry,
            &roots,
            &cleaner_core::Rules::default(),
            &|_| false,
            |_| {},
        );
        let shader = plan
            .groups
            .iter()
            .find(|group| group.app == "NVIDIA" && group.label.starts_with("Shader"))
            .unwrap();
        assert_eq!(shader.bytes, 100);
        assert!(!shader.on);

        assert_eq!(plan.selected, 8);
        assert_eq!(plan.total_bytes, 2780);

        // Groups are sorted by app then label.
        let names: Vec<&str> = plan.groups.iter().map(|g| g.app.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }
}
