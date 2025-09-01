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
	Category    string // e.g., "Cache", "Logs", "Temp", "Updates"
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
			"GPUCache",
			"DawnWebGPUCache",
			"DawnGraphiteCache",
			filepath.Join("Service Worker", "CacheStorage"),
			filepath.Join("Service Worker", "ScriptCache"),
		}
		var out []string
		for _, r := range rel {
			out = append(out, filepath.Join(base, r))
		}
		return out
	}

	cefShaderCaches := func(base string) []string {
		// Often present for CEF-based apps
		rel := []string{
			"GrShaderCache",
		}
		var out []string
		for _, r := range rel {
			out = append(out, filepath.Join(base, r))
		}
		return out
	}

	var items []Item

	// NVIDIA App updates and logs
	items = append(items,
		Item{
			App:       "NVIDIA App",
			Category:  "Updates",
			Label:     "NV App/Driver OTA Artifacts (grd/nvapp)",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "UpdateFramework", "ota-artifacts", "grd"),
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "UpdateFramework", "ota-artifacts", "nvapp"),
			},
		},
		Item{
			App:       "NVIDIA App",
			Category:  "Logs",
			Label:     "NV App Logs",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "Logs"),
			},
		},
	)

	// EA/Origin logs and anti-cheat
	items = append(items,
		Item{
			App:       "EA/Origin",
			Category:  "Logs",
			Label:     "EA logs and anti-cheat",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(programData, "EA Desktop", "Logs"),
				filepath.Join(programData, "EA Logs"),
				filepath.Join(programData, "eaanticheat"),
				filepath.Join(programData, "Origin", "Logs"),
			},
		},
	)

	// Usually empty folders
	items = append(items,
		Item{
			App:       "User Folders",
			Category:  "Misc",
			Label:     "Usually empty (.cache, ansel)",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(userProfile, ".cache"),
				filepath.Join(userProfile, "ansel"),
			},
		},
	)

	// Firefox crash reports
	items = append(items,
		Item{
			App:       "Firefox",
			Category:  "Crash Reports",
			Label:     "Crash Reports",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(appData, "Mozilla", "Firefox", "Crash Reports"),
			},
		},
	)

	// Telegram cache
	items = append(items,
		Item{
			App:       "Telegram Desktop",
			Category:  "Cache",
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

	// Discord (Electron)
	items = append(items,
		Item{
			App:       "Discord",
			Category:  "Cache",
			Label:     "Discord caches (Chromium/Electron set + logs)",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(appData, "discord")),
				filepath.Join(appData, "discord", "logs"),
			),
		},
	)

	// VSCode (Electron)
	items = append(items,
		Item{
			App:       "VSCode",
			Category:  "Cache",
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

	// osu! (lazer)
	items = append(items,
		Item{
			App:       "osu! (lazer)",
			Category:  "Cache/Logs",
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
			Category:  "Cache",
			Label:     "BF2042 cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "BattlefieldGameData.kin-release.Win32", "cache"),
			},
		},
	)

	// Shader caches (NVIDIA)
	items = append(items,
		Item{
			App:       "NVIDIA Shader Cache",
			Category:  "Cache",
			Label:     "DirectX and OpenGL shader caches",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "NVIDIA", "DXCache"),
				filepath.Join(localAppData, "NVIDIA", "GLCache"),
			},
		},
	)

	// Microsoft Edge (Chromium)
	edgeBase := filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "Default")
	items = append(items,
		Item{
			App:       "Microsoft Edge",
			Category:  "Cache",
			Label:     "Edge default profile caches (Chromium set + extension caches + shader cache)",
			DefaultOn: true,
			Paths: append(
				chromiumSet(edgeBase),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "extensions_crx_cache"),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "component_crx_cache"),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "GrShaderCache"),
			),
		},
	)

	// Google Chrome (Chromium)
	chromeBase := filepath.Join(localAppData, "Google", "Chrome", "User Data", "Default")
	items = append(items,
		Item{
			App:       "Google Chrome",
			Category:  "Cache",
			Label:     "Chrome default profile caches (Chromium set + extension caches + shader cache)",
			DefaultOn: true,
			Paths: append(
				chromiumSet(chromeBase),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "extensions_crx_cache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "component_crx_cache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "GrShaderCache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "ShaderCache"),
			),
		},
	)

	// Razer App Engine / Synapse (Electron-ish)
	razerBase := filepath.Join(localAppData, "Razer", "RazerAppEngine", "User Data", "Default")
	items = append(items,
		Item{
			App:       "Razer Synapse",
			Category:  "Cache",
			Label:     "Razer AppEngine/Synapse caches (Chromium set)",
			DefaultOn: true,
			Paths: append(
				chromiumSet(razerBase),
				// They also mentioned base Service Worker caches separately; chromiumSet includes them.
			),
		},
	)

	// Temp folder
	items = append(items,
		Item{
			App:       "Windows Temp",
			Category:  "Temp",
			Label:     "User Temp folder",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Temp"),
			},
		},
	)

	// NVIDIA App CEF cache
	nvCefBase := filepath.Join(localAppData, "NVIDIA Corporation", "NVIDIA App", "CefCache")
	items = append(items,
		Item{
			App:       "NVIDIA App",
			Category:  "CEF Cache",
			Label:     "NVIDIA App CEF caches",
			DefaultOn: true,
			Paths: append(
				cefShaderCaches(nvCefBase),
				chromiumSet(nvCefBase)...,
			),
		},
	)

	// Firefox caches (profile-dependent): use globs for profile
	items = append(items,
		Item{
			App:      "Firefox",
			Category: "Cache",
			Label:    "Firefox profile caches (cache2/startupCache/jumpListCache)",
			// Use *default-release to catch the main profile. Will be resolved by globbing later.
			DefaultOn: true,
			Globs: []string{
				filepath.Join(localAppData, "Mozilla", "Firefox", "Profiles", "*default-release", "cache2"),
				filepath.Join(localAppData, "Mozilla", "Firefox", "Profiles", "*default-release", "startupCache"),
				filepath.Join(localAppData, "Mozilla", "Firefox", "Profiles", "*default-release", "jumpListCache"),
			},
		},
	)

	// Steam cache (CEF/Electron-ish for UI)
	items = append(items,
		Item{
			App:       "Steam",
			Category:  "Cache",
			Label:     "Steam HTML cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Steam", "htmlcache", "Cache"),
			},
		},
	)

	// EA Desktop (CEF)
	eaCEFBase := filepath.Join(localAppData, "Electronic Arts", "EA Desktop", "CEF", "BrowserCache", "EADesktop", "Cache")
	items = append(items,
		Item{
			App:       "EA Desktop",
			Category:  "CEF Cache",
			Label:     "EA Desktop CEF caches",
			DefaultOn: true,
			Paths: append(
				// Use the Cache directory as the base for Chromium-style caches
				chromiumSet(eaCEFBase),
			),
		},
	)

	// Battle.net
	items = append(items,
		Item{
			App:       "Battle.net",
			Category:  "Cache",
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
			Category:  "Cache",
			Label:     "Other caches (EA Desktop helpers, VLC, generic 'cache')",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "EADesktop", "cache"),
				filepath.Join(localAppData, "Link2EA", "cache"),
				filepath.Join(localAppData, "cache"),
				filepath.Join(localAppData, "D3DSCache"),
				filepath.Join(localAppData, "EALaunchHelper", "cache"),
				filepath.Join(localAppData, "vlc", "cache"),
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
