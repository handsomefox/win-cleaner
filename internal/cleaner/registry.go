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

// BuildRegistry returns the inbuilt registry.
// All paths are constructed from environment variables to avoid hardcoded usernames.
// It also expands typical Chromium/Electron/CEF cache subfolders where needed.
func BuildRegistry() Registry {
	// Env helpers
	programData := os.Getenv("PROGRAMDATA")
	localAppData := os.Getenv("LOCALAPPDATA")
	appData := os.Getenv("APPDATA")
	userProfile := os.Getenv("USERPROFILE")

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
	items = append(items, Item{
		App:       "NVIDIA App",
		Label:     "updates/logs/shaders + CEF",
		DefaultOn: true,
		Paths: append(
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
			// Shader cache
			filepath.Join(localAppData, "NVIDIA", "DXCache"),
			filepath.Join(localAppData, "NVIDIA", "GLCache"),
		),
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
