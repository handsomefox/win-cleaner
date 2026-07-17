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
    item(app, "all profiles cache", true)
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

/// Cache directories shared by VS Code and editors derived from it.
fn vscode_set(base: &Path) -> Vec<PathBuf> {
    chromium_set(base)
        .into_iter()
        .chain([
            base.join("CachedData"),
            base.join("CachedExtensionVSIXs"),
            base.join("logs"),
        ])
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

    let mut items = vec![
        chromium_browser(
            "Chrome",
            &local.join("Google").join("Chrome").join("User Data"),
        ),
        chromium_browser(
            "Chrome Beta",
            &local.join("Google").join("Chrome Beta").join("User Data"),
        ),
        chromium_browser(
            "Chrome Dev",
            &local.join("Google").join("Chrome Dev").join("User Data"),
        ),
        chromium_browser(
            "Chrome Canary",
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
        item("Firefox", "cache + crash reports", true)
            .paths([roaming
                .join("Mozilla")
                .join("Firefox")
                .join("Crash Reports")])
            .globs([
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
        item("Brave", "all profiles cache", true)
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
        item("Opera", "cache", true).paths(chromium_set(
            &local.join("Opera Software").join("Opera Stable"),
        )),
        item("Vivaldi", "all profiles cache", true).globs(chromium_profile_globs(
            &local.join("Vivaldi").join("User Data"),
        )),
        chromium_browser("Chromium", &local.join("Chromium").join("User Data")),
        item("Opera GX", "cache", true)
            .paths(
                chromium_set(&local.join("Opera Software").join("Opera GX Stable"))
                    .into_iter()
                    .chain(chromium_set(
                        &roaming.join("Opera Software").join("Opera GX Stable"),
                    )),
            )
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
        item("Thunderbird", "all profiles cache + crash reports", true)
            .paths([roaming.join("Thunderbird").join("Crash Reports")])
            .globs([
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
        item("Discord", "cache + logs", true).paths(
            chromium_set(&roaming.join("discord"))
                .into_iter()
                .chain([roaming.join("discord").join("logs")]),
        ),
        item("Slack", "cache + logs", true).paths(
            electron_set(&roaming.join("Slack"))
                .into_iter()
                .chain([roaming.join("Slack").join("logs")]),
        ),
        item("Teams (classic)", "cache + logs", true).paths([
            roaming.join("Microsoft").join("Teams").join("Cache"),
            roaming.join("Microsoft").join("Teams").join("Code Cache"),
            roaming.join("Microsoft").join("Teams").join("GPUCache"),
            roaming
                .join("Microsoft")
                .join("Teams")
                .join("Service Worker")
                .join("CacheStorage"),
            roaming.join("Microsoft").join("Teams").join("logs"),
        ]),
        item("Teams (new)", "local cache", true).paths([local
            .join("Packages")
            .join("MSTeams_8wekyb3d8bbwe")
            .join("LocalCache")
            .join("Microsoft")
            .join("MSTeams")]),
        item("Zoom", "cache + logs", true).paths([
            roaming.join("Zoom").join("data").join("Cache"),
            roaming.join("Zoom").join("logs"),
        ]),
        item("Telegram", "cache/media/temp/dumps", true)
            .paths([
                roaming.join("Telegram Desktop").join("tdata").join("temp"),
                roaming.join("Telegram Desktop").join("tdata").join("dumps"),
            ])
            .globs([
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
        item("WhatsApp", "cache", true).paths([local.join("WhatsApp").join("Cache")]),
        item("Signal", "cache + logs", true).paths(
            electron_set(&roaming.join("Signal"))
                .into_iter()
                .chain([roaming.join("Signal").join("logs")]),
        ),
        item("Steam", "HTML cache", true)
            .paths(chromium_set(&local.join("Steam").join("htmlcache"))),
        item("Battle.net", "browser caches + logs", true).paths(
            chromium_set(
                &local
                    .join("Battle.net")
                    .join("BrowserCaches")
                    .join("common"),
            )
            .into_iter()
            .chain([
                local.join("Battle.net").join("Cache"),
                local.join("Battle.net").join("Logs"),
            ]),
        ),
        item("Epic Games Launcher", "webcache", true)
            .paths([local
                .join("EpicGamesLauncher")
                .join("Saved")
                .join("webcache")])
            .globs([local
                .join("EpicGamesLauncher")
                .join("Saved")
                .join("webcache_*")]),
        item("GOG Galaxy", "web cache", true)
            .paths([program_data.join("GOG.com").join("Galaxy").join("webcache")]),
        item("EA/Origin", "logs + CEF cache", true).paths(
            chromium_set(
                &local
                    .join("Electronic Arts")
                    .join("EA Desktop")
                    .join("CEF")
                    .join("BrowserCache")
                    .join("EADesktop")
                    .join("Cache"),
            )
            .into_iter()
            .chain([
                program_data.join("EA Desktop").join("Logs"),
                program_data.join("EA Logs"),
                program_data.join("Origin").join("Logs"),
                local.join("EADesktop").join("cache"),
                local.join("Link2EA").join("cache"),
                local.join("EALaunchHelper").join("cache"),
            ]),
        ),
        item("Rockstar Games Launcher", "cache + logs", true).paths([
            local.join("Rockstar Games").join("Launcher").join("Cache"),
            local
                .join("Rockstar Games")
                .join("Launcher")
                .join("webcache"),
            local.join("Rockstar Games").join("Launcher").join("Logs"),
        ]),
        item("Battlefield 2042", "cache", true).paths([local
            .join("BattlefieldGameData.kin-release.Win32")
            .join("cache")]),
        item("osu! (lazer)", "cache + logs", true).paths([
            roaming.join("osu").join("cache"),
            roaming.join("osu").join("logs"),
        ]),
        item("VSCode", "cache + logs", true).paths(vscode_set(&roaming.join("Code"))),
        item("Cursor", "cache + logs", true).paths(vscode_set(&roaming.join("Cursor"))),
        item("VSCodium", "cache + logs", true).paths(vscode_set(&roaming.join("VSCodium"))),
        item("GitHub Desktop", "cache + logs", true).globs([
            roaming.join("GitHub Desktop").join("*Cache"),
            roaming.join("GitHub Desktop").join("logs"),
            roaming.join("GitHubDesktop").join("*Cache"),
            roaming.join("GitHubDesktop").join("logs"),
        ]),
        item("Postman", "cache + logs", true).paths(
            electron_set(&roaming.join("Postman")).into_iter().chain([
                roaming
                    .join("Postman")
                    .join("Partitions")
                    .join("postman")
                    .join("GPUCache"),
                roaming.join("Postman").join("logs"),
            ]),
        ),
        item("Obsidian", "cache + logs", true)
            .paths(electron_set(&roaming.join("obsidian")))
            .globs([roaming.join("obsidian").join("*.log")]),
        item("Android Studio", "logs", true)
            .globs([local.join("Google").join("AndroidStudio*").join("log")]),
        item("Android Studio", "IDE caches", false)
            .globs([local.join("Google").join("AndroidStudio*").join("caches")]),
        item("JetBrains", "IDE caches", true)
            .globs([local.join("JetBrains").join("*").join("caches")]),
        item("JetBrains", "IDE logs", false).globs([local.join("JetBrains").join("*").join("log")]),
        item("npm", "package cache", true).paths([local.join("npm-cache")]),
        item("Yarn", "package cache", true).paths([local.join("Yarn").join("Cache")]),
        item("Go modules", "module download cache", false).paths([profile
            .join("go")
            .join("pkg")
            .join("mod")
            .join("cache")]),
        item("Cargo", "registry cache", false)
            .paths([profile.join(".cargo").join("registry").join("cache")]),
        item("Cargo", "git cache", false).paths([profile.join(".cargo").join("git").join("db")]),
        item("Gradle", "build cache", false).paths([profile.join(".gradle").join("caches")]),
        item("Maven", "local repository", false).paths([profile.join(".m2").join("repository")]),
        item("NuGet", "packages cache", true).paths([profile.join(".nuget").join("packages")]),
        item("pip", "download cache", false).paths([local.join("pip").join("Cache")]),
        item("pnpm", "store cache", false).paths([local.join("pnpm-cache")]),
        item("uv", "package cache", false).paths([local.join("uv").join("cache")]),
        item("Bun", "install cache", false)
            .paths([profile.join(".bun").join("install").join("cache")]),
        item("Cypress", "downloaded browser binaries", false)
            .paths([local.join("Cypress").join("Cache")]),
        item("Playwright", "downloaded browser binaries", false)
            .paths([local.join("ms-playwright")]),
        item("Visual Studio", "component model cache", false).globs([local
            .join("Microsoft")
            .join("VisualStudio")
            .join("*")
            .join("ComponentModelCache")]),
        item("Unity", "GI cache", false).paths([profile
            .join("AppData")
            .join("LocalLow")
            .join("Unity")
            .join("Caches")]),
        item("NVIDIA", "CEF cache + OTA + logs", true).paths(
            chromium_set(
                &local
                    .join("NVIDIA Corporation")
                    .join("NVIDIA App")
                    .join("CefCache"),
            )
            .into_iter()
            .chain([
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
                program_data
                    .join("NVIDIA Corporation")
                    .join("NVIDIA App")
                    .join("Logs"),
            ]),
        ),
        item("NVIDIA", "shader cache (DX + GL)", false).paths([
            local.join("NVIDIA").join("DXCache"),
            local.join("NVIDIA").join("GLCache"),
        ]),
        item("AMD", "shader cache", false).paths([
            local.join("AMD").join("DxCache"),
            local.join("AMD").join("VkCache"),
            local.join("AMD").join("OglCache"),
        ]),
        item("Razer Synapse", "cache", true).paths(chromium_set(
            &local
                .join("Razer")
                .join("RazerAppEngine")
                .join("User Data")
                .join("Default"),
        )),
        item("Spotify", "streaming cache", true)
            .paths([
                local.join("Spotify").join("Storage"),
                local.join("Spotify").join("Data"),
            ])
            .globs(
                [
                    local
                        .join("Packages")
                        .join("SpotifyAB.SpotifyMusic_*")
                        .join("LocalCache")
                        .join("Spotify")
                        .join("Data"),
                    local
                        .join("Packages")
                        .join("SpotifyAB.SpotifyMusic_*")
                        .join("LocalCache")
                        .join("Spotify")
                        .join("Storage"),
                ]
                .into_iter()
                .chain(chromium_set(
                    &local
                        .join("Packages")
                        .join("SpotifyAB.SpotifyMusic_*")
                        .join("LocalCache")
                        .join("Spotify"),
                )),
            ),
        item("OBS Studio", "logs + crashes + browser cache", true).paths([
            roaming.join("obs-studio").join("logs"),
            roaming.join("obs-studio").join("crashes"),
            roaming
                .join("obs-studio")
                .join("plugin_config")
                .join("obs-browser")
                .join("cache"),
        ]),
        item("Adobe", "media cache", false).paths([roaming
            .join("Adobe")
            .join("Common")
            .join("Media Cache Files")]),
        item("Blender", "cache", true).globs([roaming
            .join("Blender Foundation")
            .join("Blender")
            .join("*")
            .join("cache")]),
        item("Figma", "desktop cache", true).paths([roaming.join("Figma").join("Desktop")]),
        item("Notion", "cache", true).paths(electron_set(&roaming.join("Notion"))),
        item("Vortex", "cache", true).paths(chromium_set(&roaming.join("Vortex"))),
        item("qBittorrent", "logs", true).paths([local.join("qBittorrent").join("Logs")]),
        item("PowerToys", "logs", true).globs([
            local.join("Microsoft").join("PowerToys").join("*.log"),
            local
                .join("Microsoft")
                .join("PowerToys")
                .join("*")
                .join("Logs"),
        ]),
        item("Riot Client", "logs + crash reports", true)
            .paths([local.join("Riot Games").join("Riot Client").join("Logs")])
            .globs([local
                .join("Riot Games")
                .join("Riot Client")
                .join("Crashes")
                .join("Riot Client *")]),
        item("Minecraft", "logs + crash data", true).paths([
            roaming.join(".minecraft").join("logs"),
            roaming.join(".minecraft").join("crash-reports"),
            roaming.join(".minecraft").join("debug"),
        ]),
        item("Roblox", "logs", true)
            .paths([local.join("Roblox").join("logs")])
            .globs([local
                .join("Packages")
                .join("ROBLOXCORPORATION.ROBLOX_*")
                .join("LocalState")
                .join("logs")]),
        item("Roblox", "download caches", false).paths([
            local.join("Roblox").join("Downloads"),
            program_data.join("Roblox").join("Downloads"),
        ]),
        item("Windows", "Temp folder", true).paths([local.join("Temp")]),
        item("Windows", "internet cache (IE/legacy)", true)
            .paths([local.join("Microsoft").join("Windows").join("INetCache")]),
        item("Windows", "delivery optimization cache", true).paths([program_data
            .join("Microsoft")
            .join("Windows")
            .join("DeliveryOptimization")
            .join("Cache")]),
        item("Windows", "thumbnail + icon cache", true).globs([
            local
                .join("Microsoft")
                .join("Windows")
                .join("Explorer")
                .join("thumbcache*.db"),
            local
                .join("Microsoft")
                .join("Windows")
                .join("Explorer")
                .join("iconcache*"),
        ]),
        item("Windows Error Reporting", "report archives", true).paths([
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
        item("Crash dumps", "local crash dumps", true).paths([local.join("CrashDumps")]),
        item("Misc", "misc caches", true).paths([
            local.join("cache"),
            local.join("D3DSCache"),
            local.join("vlc").join("cache"),
            profile.join(".cache"),
            profile.join("ansel"),
        ]),
    ];

    if let Some(x86) = roots.program_files_x86.as_deref() {
        items.push(
            item("Ubisoft Connect", "cache + logs", true).paths([
                x86.join("Ubisoft")
                    .join("Ubisoft Game Launcher")
                    .join("cache"),
                x86.join("Ubisoft")
                    .join("Ubisoft Game Launcher")
                    .join("logs"),
            ]),
        );
    }
    if let Some(system_root) = roots.system_root.as_deref() {
        items.push(
            item("Windows", "update download cache", true)
                .paths([system_root.join("SoftwareDistribution").join("Download")]),
        );
        items.push(item("Windows", "prefetch", false).paths([system_root.join("Prefetch")]));
    }

    Registry { items }
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
    fn full_registry_shape() {
        let mut roots = test_roots(Path::new("/base"));
        roots.system_root = Some(PathBuf::from("/base/Windows"));
        let registry = build_registry(&roots);
        assert_eq!(registry.items.len(), 82);

        let chrome = registry
            .items
            .iter()
            .find(|item| item.app == "Chrome")
            .unwrap();
        assert!(chrome.default_on);
        assert_eq!(chrome.paths.len(), 4);
        assert_eq!(chrome.globs.len(), 7);

        let telegram = registry
            .items
            .iter()
            .find(|item| item.app == "Telegram")
            .unwrap();
        assert_eq!(telegram.paths.len(), 2);
        assert!(telegram.globs[0].to_string_lossy().contains("user_data*"));

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
            .find(|i| i.app == "Windows" && i.label == "prefetch")
            .unwrap();
        assert!(!prefetch.default_on);
        assert_eq!(prefetch.paths.len(), 1);

        let ubisoft = registry
            .items
            .iter()
            .find(|item| item.app == "Ubisoft Connect")
            .unwrap();
        assert_eq!(ubisoft.paths.len(), 2);

        for (app, root) in [
            ("Chrome", "/base/Local/Google/Chrome/User Data"),
            ("Chrome Beta", "/base/Local/Google/Chrome Beta/User Data"),
            ("Chrome Dev", "/base/Local/Google/Chrome Dev/User Data"),
            ("Chrome Canary", "/base/Local/Google/Chrome SxS/User Data"),
            (
                "Chrome for Testing",
                "/base/Local/Google/Chrome for Testing/User Data",
            ),
            ("Edge", "/base/Local/Microsoft/Edge/User Data"),
            ("Chromium", "/base/Local/Chromium/User Data"),
        ] {
            let browser = registry.items.iter().find(|item| item.app == app).unwrap();
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

        let firefox = registry.items.iter().find(|i| i.app == "Firefox").unwrap();
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
            .find(|i| i.app == "Thunderbird")
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

        for app in ["Android Studio", "uv", "Bun", "Cypress", "Playwright"] {
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
                !item.paths.is_empty() || !item.globs.is_empty(),
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
            for path in item.paths.iter().chain(&item.globs) {
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
        assert_eq!(registry.items.len(), 79);
        assert!(
            !registry
                .items
                .iter()
                .any(|item| item.app == "Ubisoft Connect")
        );
        assert!(!registry.items.iter().any(|item| item.label == "prefetch"));
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
        let plan = build_plan(&registry, &roots, |_| updates += 1);
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

        for (app, bytes) in [
            ("Chrome Beta", 110),
            ("Firefox", 250),
            ("Opera GX", 200),
            ("Thunderbird", 610),
            ("PowerToys", 50),
            ("Roblox", 60),
        ] {
            let group = plan
                .groups
                .iter()
                .find(|group| group.app == app && (app != "Roblox" || group.label == "logs"))
                .unwrap();
            assert_eq!(group.bytes, bytes, "{app}");
            assert!(group.on, "{app}");
        }

        let uv = plan.groups.iter().find(|group| group.app == "uv").unwrap();
        assert_eq!(uv.bytes, 400);
        assert!(!uv.on);

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
        let plan = build_plan(&registry, &roots, |_| {});
        let shader = plan
            .groups
            .iter()
            .find(|group| group.app == "NVIDIA" && group.label.starts_with("shader"))
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
