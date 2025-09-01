package cleaner

import (
	"os"
	"path/filepath"
	"strings"
)

// Registry holds the inbuilt catalog of apps and their cache/log paths.
// Update this file to add/remove paths safely.
type Registry struct {
	Items []Item
}

// Item groups a set of paths under an App and Category. Each item is individually
// selectable in interactive mode.
type Item struct {
	App         string
	Label       string // human-friendly label for the group
	Paths       []string
	Globs       []string // optional glob patterns to expand at runtime
	DefaultOn   bool     // default selection is on
	Description string   // optional tooltip/description
}

// BuildRegistry returns the inbuilt registry.
// All paths are constructed from environment variables to avoid hardcoded usernames.
// It also expands typical Chromium/Electron/CEF cache subfolders where needed.
func BuildRegistry() Registry {
	// Env helpers
	programData := os.Getenv("PROGRAMDATA")
	localAppData := os.Getenv("LOCALAPPDATA")
	appData := os.Getenv("APPDATA")
	userProfile := os.Getenv("USERPROFILE")

	// Guard: basic sanity to prevent catastrophic mistakes if envs are empty.
	// If we can't detect essential envs, we fall back to safe no-op.
	if programData == "" || localAppData == "" || appData == "" || userProfile == "" {
		return Registry{Items: nil}
	}

	// Helper builders
	chromiumSet := func(base string) []string {
		// Common Chromium/Electron/CEF cache directories
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

	var items []Item

	// NVIDIA
	items = append(items,
		Item{
			App:       "NVIDIA App",
			Label:     "NV App/Driver OTA Artifacts (grd/nvapp)/Logs/Shader cache",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(localAppData, "NVIDIA Corporation", "NVIDIA App", "CefCache")),
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "UpdateFramework", "ota-artifacts", "grd"),
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "UpdateFramework", "ota-artifacts", "nvapp"),
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "Logs"),
				filepath.Join(localAppData, "NVIDIA", "DXCache"),
				filepath.Join(localAppData, "NVIDIA", "GLCache"),
			),
		},
	)

	// EA
	items = append(items,
		Item{
			App:       "EA/Origin",
			Label:     "EA logs and anti-cheat",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(localAppData, "Electronic Arts", "EA Desktop", "CEF", "BrowserCache", "EADesktop", "Cache")),
				filepath.Join(programData, "EA Desktop", "Logs"),
				filepath.Join(programData, "EA Logs"),
				filepath.Join(programData, "eaanticheat"),
				filepath.Join(programData, "Origin", "Logs"),
				filepath.Join(localAppData, "EADesktop", "cache"),
				filepath.Join(localAppData, "Link2EA", "cache"),
				filepath.Join(localAppData, "EALaunchHelper", "cache"),
			),
		},
	)

	// Firefox
	items = append(items,
		Item{
			App:       "Firefox",
			Label:     "Cache, crash reports",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(appData, "Mozilla", "Firefox", "Crash Reports"),
			},
			Globs: []string{
				filepath.Join(localAppData, "Mozilla", "Firefox", "Profiles", "*default-release", "cache2"),
				filepath.Join(localAppData, "Mozilla", "Firefox", "Profiles", "*default-release", "startupCache"),
				filepath.Join(localAppData, "Mozilla", "Firefox", "Profiles", "*default-release", "jumpListCache"),
			},
		},
	)

	// Telegram
	items = append(items,
		Item{
			App:       "Telegram Desktop",
			Label:     "Cache, media cache, temp, dumps",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(appData, "Telegram Desktop", "tdata", "user_data", "cache"),
				filepath.Join(appData, "Telegram Desktop", "tdata", "user_data", "media_cache"),
				filepath.Join(appData, "Telegram Desktop", "tdata", "temp"),
				filepath.Join(appData, "Telegram Desktop", "tdata", "dumps"),
			},
		},
	)

	// Discord
	items = append(items,
		Item{
			App:       "Discord",
			Label:     "Discord caches (Chromium/Electron set + logs)",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(appData, "discord")),
				filepath.Join(appData, "discord", "logs"),
			),
		},
	)

	// VSCode
	items = append(items,
		Item{
			App:       "VSCode",
			Label:     "VSCode caches and logs",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(appData, "Code")),
				filepath.Join(appData, "Code", "CachedData"),
				filepath.Join(appData, "Code", "CachedExtensionVSIXs"),
				filepath.Join(appData, "Code", "logs"),
			),
		},
	)

	// osu!(lazer)
	items = append(items,
		Item{
			App:       "osu! (lazer)",
			Label:     "osu! caches and logs",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(appData, "osu", "cache"),
				filepath.Join(appData, "osu", "logs"),
			},
		},
	)

	// Battlefield 2042
	items = append(items,
		Item{
			App:       "Battlefield 2042",
			Label:     "BF2042 cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "BattlefieldGameData.kin-release.Win32", "cache"),
			},
		},
	)

	// Microsoft Edge (Chromium)
	items = append(items,
		Item{
			App:       "Microsoft Edge",
			Label:     "Edge default profile caches (Chromium set + extension caches + shader cache)",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "Default")),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "extensions_crx_cache"),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "component_crx_cache"),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "GrShaderCache"),
			),
		},
	)

	// Google Chrome (Chromium)
	items = append(items,
		Item{
			App:       "Google Chrome",
			Label:     "Chrome default profile caches (Chromium set + extension caches + shader cache)",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(localAppData, "Google", "Chrome", "User Data", "Default")),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "extensions_crx_cache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "component_crx_cache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "GrShaderCache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "ShaderCache"),
			),
		},
	)

	// Razer
	items = append(items,
		Item{
			App:       "Razer Synapse",
			Label:     "Razer AppEngine/Synapse caches (Chromium set)",
			DefaultOn: true,
			Paths:     chromiumSet(filepath.Join(localAppData, "Razer", "RazerAppEngine", "User Data", "Default")),
		},
	)

	// Steam cache (CEF/Electron-ish for UI)
	items = append(items,
		Item{
			App:       "Steam",
			Label:     "Steam HTML cache",
			DefaultOn: true,
			Paths:     chromiumSet(filepath.Join(localAppData, "Steam", "htmlcache")),
		},
	)

	// Battle.net
	items = append(items,
		Item{
			App:       "Battle.net",
			Label:     "Battle.net browser caches and logs",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(localAppData, "Battle.net", "BrowserCaches", "common")),
				filepath.Join(localAppData, "Battle.net", "Cache"),
				filepath.Join(localAppData, "Battle.net", "Logs"),
			),
		},
	)

	// Others
	items = append(items,
		Item{
			App:       "Misc",
			Label:     "Other caches (EA Desktop helpers, VLC, generic 'cache')",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "cache"),
				filepath.Join(localAppData, "D3DSCache"),
				filepath.Join(localAppData, "vlc", "cache"),
				filepath.Join(userProfile, ".cache"),
				filepath.Join(userProfile, "ansel"),
				filepath.Join(localAppData, "Temp"),
			},
		},
	)

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
