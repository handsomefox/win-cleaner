package cleaner

import (
	"os"
	"path/filepath"
	"strings"
)

type Registry struct{ Items []Item }

// Item groups a set of paths under an App and Category. Each item is individually
// selectable in interactive mode.
type Item struct {
	App       string
	Label     string // human-friendly label for the group
	Paths     []string
	Globs     []string // optional glob patterns to expand at runtime
	DefaultOn bool     // default selection is on
}

type RegistryConfig struct {
	SkipShaderCache bool
}

// BuildRegistry returns the inbuilt registry.
// All paths are constructed from environment variables to avoid hardcoded usernames.
// It also expands typical Chromium/Electron/CEF cache subfolders where needed.
func BuildRegistry(rc RegistryConfig) Registry {
	// Env helpers
	programData := os.Getenv("PROGRAMDATA")
	localAppData := os.Getenv("LOCALAPPDATA")
	appData := os.Getenv("APPDATA")
	userProfile := os.Getenv("USERPROFILE")
	programFilesX86 := os.Getenv("ProgramFiles(x86)")

	// Guard: basic sanity; if we can't detect essential envs, no-op.
	if programData == "" || localAppData == "" || appData == "" || userProfile == "" {
		return Registry{Items: nil}
	}

	// Helper: common Chromium/Electron/CEF per-profile cache folders
	chromiumSet := func(base string) []string {
		rel := []string{
			"Cache",
			"Code Cache",
			"DawnGraphiteCache",
			"DawnWebGPUCache",
			"GPUCache",
			"GrShaderCache",
			"ShaderCache",
			filepath.Join("Service Worker", "CacheStorage"),
			filepath.Join("Service Worker", "ScriptCache"),
		}
		var out []string
		for _, r := range rel {
			out = append(out, filepath.Join(base, r))
		}
		return out
	}

	items := make([]Item, 0, 64)

	// NVIDIA App (updates/logs + shader + CEF)
	nvidiaAppPaths := append(
		// CEF cache under user local
		chromiumSet(filepath.Join(
			localAppData, "NVIDIA Corporation", "NVIDIA App", "CefCache",
		)),
		// OTA artifacts + logs
		filepath.Join(
			programData, "NVIDIA Corporation", "NVIDIA App",
			"UpdateFramework", "ota-artifacts", "grd",
		),
		filepath.Join(
			programData, "NVIDIA Corporation", "NVIDIA App",
			"UpdateFramework", "ota-artifacts", "nvapp",
		),
		filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "Logs"),
	)

	// Shader cache
	if !rc.SkipShaderCache {
		nvidiaAppPaths = append(nvidiaAppPaths,
			filepath.Join(localAppData, "NVIDIA", "DXCache"),
			filepath.Join(localAppData, "NVIDIA", "GLCache"),
		)
	}

	items = append(items, Item{
		App:       "NVIDIA App",
		Label:     "updates/logs/shaders + CEF",
		DefaultOn: true,
		Paths:     nvidiaAppPaths,
	})

	// EA / Origin + EA Desktop CEF
	items = append(items, Item{
		App:       "EA/Origin",
		Label:     "logs + anti-cheat + CEF cache",
		DefaultOn: true,
		Paths: append(
			// EA Desktop CEF cache
			chromiumSet(filepath.Join(
				localAppData, "Electronic Arts", "EA Desktop", "CEF",
				"BrowserCache", "EADesktop", "Cache",
			)),
			// Logs
			filepath.Join(programData, "EA Desktop", "Logs"),
			filepath.Join(programData, "EA Logs"),
			filepath.Join(programData, "eaanticheat"),
			filepath.Join(programData, "Origin", "Logs"),
			// Other EA caches
			filepath.Join(localAppData, "EADesktop", "cache"),
			filepath.Join(localAppData, "Link2EA", "cache"),
			filepath.Join(localAppData, "EALaunchHelper", "cache"),
		),
	})

	// Firefox (cache + crash reports)
	items = append(items, Item{
		App:       "Firefox",
		Label:     "cache + crash reports",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(appData, "Mozilla", "Firefox", "Crash Reports"),
		},
		Globs: []string{
			filepath.Join(
				localAppData, "Mozilla", "Firefox", "Profiles",
				"*default-release", "cache2",
			),
			filepath.Join(
				localAppData, "Mozilla", "Firefox", "Profiles",
				"*default-release", "startupCache",
			),
			filepath.Join(
				localAppData, "Mozilla", "Firefox", "Profiles",
				"*default-release", "jumpListCache",
			),
		},
	})

	// Telegram Desktop
	items = append(items, Item{
		App:       "Telegram",
		Label:     "cache/media/temp/dumps",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(appData, "Telegram Desktop", "tdata", "user_data", "cache"),
			filepath.Join(appData, "Telegram Desktop", "tdata", "user_data", "media_cache"),
			filepath.Join(appData, "Telegram Desktop", "tdata", "temp"),
			filepath.Join(appData, "Telegram Desktop", "tdata", "dumps"),
		},
	})

	// Vortex
	items = append(items, Item{
		App:       "Vortex",
		Label:     "cache",
		DefaultOn: true,
		Paths:     chromiumSet(filepath.Join(appData, "Vortex")),
	})

	// Discord (Electron)
	items = append(items, Item{
		App:       "Discord",
		Label:     "cache + logs",
		DefaultOn: true,
		Paths: append(
			chromiumSet(filepath.Join(appData, "discord")),
			filepath.Join(appData, "discord", "logs"),
		),
	})

	// Microsoft Teams (classic)
	items = append(items, Item{
		App:       "Teams (classic)",
		Label:     "cache + logs",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(appData, "Microsoft", "Teams", "Cache"),
			filepath.Join(appData, "Microsoft", "Teams", "Code Cache"),
			filepath.Join(appData, "Microsoft", "Teams", "GPUCache"),
			filepath.Join(appData, "Microsoft", "Teams", "Service Worker", "CacheStorage"),
			filepath.Join(appData, "Microsoft", "Teams", "logs"),
		},
	})

	// Microsoft Teams (new)
	items = append(items, Item{
		App:       "Teams (new)",
		Label:     "local cache",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "Packages", "MSTeams_8wekyb3d8bbwe", "LocalCache", "Microsoft", "MSTeams"),
		},
	})

	// Slack (Electron)
	items = append(items, Item{
		App:       "Slack",
		Label:     "cache + logs",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(appData, "Slack", "Cache"),
			filepath.Join(appData, "Slack", "Code Cache"),
			filepath.Join(appData, "Slack", "GPUCache"),
			filepath.Join(appData, "Slack", "Service Worker", "CacheStorage"),
			filepath.Join(appData, "Slack", "logs"),
		},
	})

	// Zoom
	items = append(items, Item{
		App:       "Zoom",
		Label:     "logs",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(appData, "Zoom", "logs"),
		},
	})

	// Notion (Electron)
	items = append(items, Item{
		App:       "Notion",
		Label:     "cache",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(appData, "Notion", "Cache"),
			filepath.Join(appData, "Notion", "Code Cache"),
			filepath.Join(appData, "Notion", "GPUCache"),
			filepath.Join(appData, "Notion", "Service Worker", "CacheStorage"),
		},
	})

	// VSCode (Electron)
	items = append(items, Item{
		App:       "VSCode",
		Label:     "cache + logs",
		DefaultOn: true,
		Paths: append(
			chromiumSet(filepath.Join(appData, "Code")),
			filepath.Join(appData, "Code", "CachedData"),
			filepath.Join(appData, "Code", "CachedExtensionVSIXs"),
			filepath.Join(appData, "Code", "logs"),
		),
	})

	// osu! (lazer)
	items = append(items, Item{
		App:       "osu! (lazer)",
		Label:     "cache + logs",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(appData, "osu", "cache"),
			filepath.Join(appData, "osu", "logs"),
		},
	})

	// Battlefield 2042
	items = append(items, Item{
		App:       "Battlefield 2042",
		Label:     "cache",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "BattlefieldGameData.kin-release.Win32", "cache"),
		},
	})

	// Microsoft Edge (Chromium) — all profiles + root caches
	items = append(items, Item{
		App:       "Edge",
		Label:     "all profiles cache",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "extensions_crx_cache"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "component_crx_cache"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "GrShaderCache"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "ShaderCache"),
		},
		Globs: []string{
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "*", "Cache"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "*", "Code Cache"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "*", "GPUCache"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "*", "DawnWebGPUCache"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "*", "DawnGraphiteCache"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "*", "Service Worker", "CacheStorage"),
			filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "*", "Service Worker", "ScriptCache"),
		},
	})

	// Google Chrome — all profiles + root caches
	items = append(items, Item{
		App:       "Chrome",
		Label:     "all profiles cache",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "extensions_crx_cache"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "component_crx_cache"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "GrShaderCache"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "ShaderCache"),
		},
		Globs: []string{
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "*", "Cache"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "*", "Code Cache"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "*", "GPUCache"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "*", "DawnWebGPUCache"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "*", "DawnGraphiteCache"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "*",
				"Service Worker", "CacheStorage"),
			filepath.Join(localAppData, "Google", "Chrome", "User Data", "*",
				"Service Worker", "ScriptCache"),
		},
	})

	// Razer Synapse (Electron)
	items = append(items, Item{
		App:       "Razer Synapse",
		Label:     "cache",
		DefaultOn: true,
		Paths: chromiumSet(filepath.Join(
			localAppData, "Razer", "RazerAppEngine", "User Data", "Default",
		)),
	})

	// Steam (CEF/Electron UI)
	items = append(items, Item{
		App:       "Steam",
		Label:     "HTML cache",
		DefaultOn: true,
		Paths:     chromiumSet(filepath.Join(localAppData, "Steam", "htmlcache")),
	})

	// Battle.net
	items = append(items, Item{
		App:       "Battle.net",
		Label:     "browser caches + logs",
		DefaultOn: true,
		Paths: append(
			chromiumSet(filepath.Join(localAppData, "Battle.net", "BrowserCaches", "common")),
			filepath.Join(localAppData, "Battle.net", "Cache"),
			filepath.Join(localAppData, "Battle.net", "Logs"),
		),
	})

	// Epic Games Launcher
	items = append(items, Item{
		App:       "Epic Games Launcher",
		Label:     "webcache",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "EpicGamesLauncher", "Saved", "webcache"),
		},
		Globs: []string{
			filepath.Join(localAppData, "EpicGamesLauncher", "Saved", "webcache_*"),
		},
	})

	// Ubisoft Connect
	ubisoftPaths := []string{}
	if programFilesX86 != "" {
		ubisoftPaths = append(ubisoftPaths,
			filepath.Join(programFilesX86, "Ubisoft", "Ubisoft Game Launcher", "cache"),
			filepath.Join(programFilesX86, "Ubisoft", "Ubisoft Game Launcher", "logs"),
		)
	}
	items = append(items, Item{
		App:       "Ubisoft Connect",
		Label:     "cache + logs",
		DefaultOn: true,
		Paths:     ubisoftPaths,
	})

	// GOG Galaxy
	items = append(items, Item{
		App:       "GOG Galaxy",
		Label:     "cache",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "GOG.com", "Galaxy"),
			filepath.Join(programData, "GOG.com", "Galaxy", "webcache"),
		},
	})

	// Rockstar Games Launcher
	items = append(items, Item{
		App:       "Rockstar Games Launcher",
		Label:     "cache + logs",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "Rockstar Games", "Launcher", "Cache"),
			filepath.Join(localAppData, "Rockstar Games", "Launcher", "webcache"),
			filepath.Join(localAppData, "Rockstar Games", "Launcher", "Logs"),
		},
	})

	// Spotify
	items = append(items, Item{
		App:       "Spotify",
		Label:     "cache/data (desktop + Store app)",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "Spotify", "Storage"),
			filepath.Join(localAppData, "Spotify", "Data"),
		},
		Globs: append(
			[]string{
				filepath.Join(localAppData, "Packages", "SpotifyAB.SpotifyMusic_*", "LocalCache", "Spotify", "Data"),
				filepath.Join(localAppData, "Packages", "SpotifyAB.SpotifyMusic_*", "LocalCache", "Spotify", "Storage"),
			},
			chromiumSet(filepath.Join(
				localAppData, "Packages", "SpotifyAB.SpotifyMusic_*", "LocalCache", "Spotify",
			))...,
		),
	})

	// NuGet cache
	items = append(items, Item{
		App:       "NuGet",
		Label:     "packages cache",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(userProfile, ".nuget", "packages"),
		},
	})

	// Windows Error Reporting (WER)
	items = append(items, Item{
		App:       "Windows Error Reporting",
		Label:     "report archives",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "Microsoft", "Windows", "WER", "ReportArchive"),
			filepath.Join(localAppData, "Microsoft", "Windows", "WER", "ReportQueue"),
		},
	})

	// Crash dumps
	items = append(items, Item{
		App:       "Crash dumps",
		Label:     "local crash dumps",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "CrashDumps"),
		},
	})

	// Windows icon/thumb caches
	items = append(items, Item{
		App:       "Icon caches",
		Label:     "thumbcache + iconcache",
		DefaultOn: true,
		Globs: []string{
			filepath.Join(localAppData, "Microsoft", "Windows", "Explorer", "thumbcache*.db"),
			filepath.Join(localAppData, "Microsoft", "Windows", "Explorer", "iconcache*"),
		},
	})

	// Windows Temp
	items = append(items, Item{
		App:       "Windows Temp",
		Label:     "Temp",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "Temp"),
		},
	})

	// Misc (small, safe caches + user dot folders)
	items = append(items, Item{
		App:       "Misc",
		Label:     "misc caches",
		DefaultOn: true,
		Paths: []string{
			filepath.Join(localAppData, "cache"),
			filepath.Join(localAppData, "D3DSCache"),
			filepath.Join(localAppData, "vlc", "cache"),
			filepath.Join(userProfile, ".cache"),
			filepath.Join(userProfile, "ansel"),
		},
	})

	return Registry{Items: items}
}

func (r Registry) Apps() []string {
	seen := map[string]struct{}{}
	var out []string
	for _, it := range r.Items {
		if _, ok := seen[it.App]; !ok {
			seen[it.App] = struct{}{}
			out = append(out, it.App)
		}
	}
	return out
}

func (r Registry) FilterInclude(apps []string) Registry {
	if len(apps) == 0 {
		return r
	}
	allow := make(map[string]struct{}, len(apps))
	for _, a := range apps {
		allow[strings.ToLower(strings.TrimSpace(a))] = struct{}{}
	}
	var out Registry
	for _, it := range r.Items {
		if _, ok := allow[strings.ToLower(it.App)]; ok {
			out.Items = append(out.Items, it)
		}
	}
	return out
}

func (r Registry) FilterExclude(apps []string) Registry {
	if len(apps) == 0 {
		return r
	}
	block := make(map[string]struct{}, len(apps))
	for _, a := range apps {
		block[strings.ToLower(strings.TrimSpace(a))] = struct{}{}
	}
	var out Registry
	for _, it := range r.Items {
		if _, ok := block[strings.ToLower(it.App)]; ok {
			continue
		}
		out.Items = append(out.Items, it)
	}
	return out
}
