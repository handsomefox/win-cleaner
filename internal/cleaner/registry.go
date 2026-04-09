package cleaner

import (
	"os"
	"path/filepath"
	"strings"
)

type Registry struct{ Items []Item }

// Item defines a single cleanup target under an App name.
// Each item is individually selectable in the GUI.
type Item struct {
	App       string
	Label     string   // human-friendly description shown in the UI
	Paths     []string // static paths (resolved at runtime from env vars)
	Globs     []string // glob patterns expanded at runtime
	DefaultOn bool     // pre-selected in the GUI
}

// BuildRegistry returns the built-in registry of cleanup targets.
// All paths are derived from Windows environment variables so no usernames
// are hardcoded. Returns an empty registry when the required env vars are
// absent (i.e. on non-Windows platforms).
func BuildRegistry() Registry {
	localAppData := os.Getenv("LOCALAPPDATA")
	appData := os.Getenv("APPDATA")
	programData := os.Getenv("PROGRAMDATA")
	userProfile := os.Getenv("USERPROFILE")
	programFilesX86 := os.Getenv("ProgramFiles(x86)")
	systemRoot := os.Getenv("SystemRoot")

	if localAppData == "" || appData == "" || programData == "" || userProfile == "" {
		return Registry{}
	}

	// chromiumSet returns the standard per-profile Chromium cache subdirectories
	// rooted at base (covers Chrome, Edge, Brave, Vivaldi, Opera, CEF apps).
	chromiumSet := func(base string) []string {
		subs := []string{
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
		out := make([]string, len(subs))
		for i, s := range subs {
			out[i] = filepath.Join(base, s)
		}
		return out
	}

	// chromiumProfileGlobs returns per-profile glob patterns for a Chromium
	// User Data directory, covering Cache and related subdirs for all profiles.
	chromiumProfileGlobs := func(userData string) []string {
		return []string{
			filepath.Join(userData, "*", "Cache"),
			filepath.Join(userData, "*", "Code Cache"),
			filepath.Join(userData, "*", "GPUCache"),
			filepath.Join(userData, "*", "DawnWebGPUCache"),
			filepath.Join(userData, "*", "DawnGraphiteCache"),
			filepath.Join(userData, "*", "Service Worker", "CacheStorage"),
			filepath.Join(userData, "*", "Service Worker", "ScriptCache"),
		}
	}

	// electronSet returns the typical Electron app cache directories rooted at base.
	electronSet := func(base string) []string {
		return []string{
			filepath.Join(base, "Cache"),
			filepath.Join(base, "Code Cache"),
			filepath.Join(base, "GPUCache"),
			filepath.Join(base, "Service Worker", "CacheStorage"),
		}
	}

	items := make([]Item, 0, 96)

	// --- Browsers ---

	items = append(items,
		Item{
			App:       "Chrome",
			Label:     "all profiles cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "extensions_crx_cache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "component_crx_cache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "GrShaderCache"),
				filepath.Join(localAppData, "Google", "Chrome", "User Data", "ShaderCache"),
			},
			Globs: chromiumProfileGlobs(filepath.Join(localAppData, "Google", "Chrome", "User Data")),
		},
		Item{
			App:       "Edge",
			Label:     "all profiles cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "extensions_crx_cache"),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "component_crx_cache"),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "GrShaderCache"),
				filepath.Join(localAppData, "Microsoft", "Edge", "User Data", "ShaderCache"),
			},
			Globs: chromiumProfileGlobs(filepath.Join(localAppData, "Microsoft", "Edge", "User Data")),
		},
		Item{
			App:       "Firefox",
			Label:     "cache + crash reports",
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
		Item{
			App:       "Brave",
			Label:     "all profiles cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "BraveSoftware", "Brave-Browser", "User Data", "GrShaderCache"),
				filepath.Join(localAppData, "BraveSoftware", "Brave-Browser", "User Data", "ShaderCache"),
			},
			Globs: chromiumProfileGlobs(filepath.Join(localAppData, "BraveSoftware", "Brave-Browser", "User Data")),
		},
		Item{
			App:       "Opera",
			Label:     "cache",
			DefaultOn: true,
			Paths:     chromiumSet(filepath.Join(localAppData, "Opera Software", "Opera Stable")),
		},
		Item{
			App:       "Vivaldi",
			Label:     "all profiles cache",
			DefaultOn: true,
			Globs:     chromiumProfileGlobs(filepath.Join(localAppData, "Vivaldi", "User Data")),
		},
	)

	// --- Communication ---

	items = append(items,
		Item{
			App:       "Discord",
			Label:     "cache + logs",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(appData, "discord")),
				filepath.Join(appData, "discord", "logs"),
			),
		},
		Item{
			App:       "Slack",
			Label:     "cache + logs",
			DefaultOn: true,
			Paths: append(
				electronSet(filepath.Join(appData, "Slack")),
				filepath.Join(appData, "Slack", "logs"),
			),
		},
		Item{
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
		},
		Item{
			App:       "Teams (new)",
			Label:     "local cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Packages", "MSTeams_8wekyb3d8bbwe", "LocalCache", "Microsoft", "MSTeams"),
			},
		},
		Item{
			App:       "Zoom",
			Label:     "cache + logs",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(appData, "Zoom", "data", "Cache"),
				filepath.Join(appData, "Zoom", "logs"),
			},
		},
		Item{
			App:       "Telegram",
			Label:     "cache/media/temp/dumps",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(appData, "Telegram Desktop", "tdata", "user_data", "cache"),
				filepath.Join(appData, "Telegram Desktop", "tdata", "user_data", "media_cache"),
				filepath.Join(appData, "Telegram Desktop", "tdata", "temp"),
				filepath.Join(appData, "Telegram Desktop", "tdata", "dumps"),
			},
		},
		Item{
			App:       "WhatsApp",
			Label:     "cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "WhatsApp", "Cache"),
			},
		},
	)

	// --- Games ---

	items = append(items,
		Item{
			App:       "Steam",
			Label:     "HTML cache",
			DefaultOn: true,
			Paths:     chromiumSet(filepath.Join(localAppData, "Steam", "htmlcache")),
		},
		Item{
			App:       "Battle.net",
			Label:     "browser caches + logs",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(localAppData, "Battle.net", "BrowserCaches", "common")),
				filepath.Join(localAppData, "Battle.net", "Cache"),
				filepath.Join(localAppData, "Battle.net", "Logs"),
			),
		},
		Item{
			App:       "Epic Games Launcher",
			Label:     "webcache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "EpicGamesLauncher", "Saved", "webcache"),
			},
			Globs: []string{
				filepath.Join(localAppData, "EpicGamesLauncher", "Saved", "webcache_*"),
			},
		},
		Item{
			App:       "Ubisoft Connect",
			Label:     "cache + logs",
			DefaultOn: true,
			Paths: func() []string {
				if programFilesX86 == "" {
					return nil
				}
				return []string{
					filepath.Join(programFilesX86, "Ubisoft", "Ubisoft Game Launcher", "cache"),
					filepath.Join(programFilesX86, "Ubisoft", "Ubisoft Game Launcher", "logs"),
				}
			}(),
		},
		Item{
			App:       "GOG Galaxy",
			Label:     "cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "GOG.com", "Galaxy"),
				filepath.Join(programData, "GOG.com", "Galaxy", "webcache"),
			},
		},
		Item{
			App:       "EA/Origin",
			Label:     "logs + anti-cheat + CEF cache",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(
					localAppData, "Electronic Arts", "EA Desktop", "CEF", "BrowserCache", "EADesktop", "Cache",
				)),
				filepath.Join(programData, "EA Desktop", "Logs"),
				filepath.Join(programData, "EA Logs"),
				filepath.Join(programData, "eaanticheat"),
				filepath.Join(programData, "Origin", "Logs"),
				filepath.Join(localAppData, "EADesktop", "cache"),
				filepath.Join(localAppData, "Link2EA", "cache"),
				filepath.Join(localAppData, "EALaunchHelper", "cache"),
			),
		},
		Item{
			App:       "Rockstar Games Launcher",
			Label:     "cache + logs",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Rockstar Games", "Launcher", "Cache"),
				filepath.Join(localAppData, "Rockstar Games", "Launcher", "webcache"),
				filepath.Join(localAppData, "Rockstar Games", "Launcher", "Logs"),
			},
		},
		Item{
			App:       "Battlefield 2042",
			Label:     "cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "BattlefieldGameData.kin-release.Win32", "cache"),
			},
		},
		Item{
			App:       "osu! (lazer)",
			Label:     "cache + logs",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(appData, "osu", "cache"),
				filepath.Join(appData, "osu", "logs"),
			},
		},
	)

	// --- Dev Tools ---

	items = append(items,
		Item{
			App:       "VSCode",
			Label:     "cache + logs",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(appData, "Code")),
				filepath.Join(appData, "Code", "CachedData"),
				filepath.Join(appData, "Code", "CachedExtensionVSIXs"),
				filepath.Join(appData, "Code", "logs"),
			),
		},
		Item{
			App:       "JetBrains",
			Label:     "IDE caches",
			DefaultOn: true,
			Globs: []string{
				filepath.Join(localAppData, "JetBrains", "*", "caches"),
			},
		},
		Item{
			App:       "JetBrains",
			Label:     "IDE logs",
			DefaultOn: false,
			Globs: []string{
				filepath.Join(localAppData, "JetBrains", "*", "log"),
			},
		},
		Item{
			App:       "npm",
			Label:     "package cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "npm-cache"),
			},
		},
		Item{
			App:       "Yarn",
			Label:     "package cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Yarn", "Cache"),
			},
		},
		Item{
			App:       "Go modules",
			Label:     "module download cache",
			DefaultOn: false,
			Paths: []string{
				filepath.Join(userProfile, "go", "pkg", "mod", "cache"),
			},
		},
		Item{
			App:       "Cargo",
			Label:     "registry cache",
			DefaultOn: false,
			Paths: []string{
				filepath.Join(userProfile, ".cargo", "registry", "cache"),
			},
		},
		Item{
			App:       "Cargo",
			Label:     "git cache",
			DefaultOn: false,
			Paths: []string{
				filepath.Join(userProfile, ".cargo", "git", "db"),
			},
		},
		Item{
			App:       "Gradle",
			Label:     "build cache",
			DefaultOn: false,
			Paths: []string{
				filepath.Join(userProfile, ".gradle", "caches"),
			},
		},
		Item{
			App:       "Maven",
			Label:     "local repository",
			DefaultOn: false,
			Paths: []string{
				filepath.Join(userProfile, ".m2", "repository"),
			},
		},
		Item{
			App:       "NuGet",
			Label:     "packages cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(userProfile, ".nuget", "packages"),
			},
		},
	)

	// --- Hardware ---

	items = append(items,
		Item{
			App:       "NVIDIA",
			Label:     "CEF cache + OTA + logs",
			DefaultOn: true,
			Paths: append(
				chromiumSet(filepath.Join(localAppData, "NVIDIA Corporation", "NVIDIA App", "CefCache")),
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "UpdateFramework", "ota-artifacts", "grd"),
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "UpdateFramework", "ota-artifacts", "nvapp"),
				filepath.Join(programData, "NVIDIA Corporation", "NVIDIA App", "Logs"),
			),
		},
		Item{
			App:       "NVIDIA",
			Label:     "shader cache (DX + GL)",
			DefaultOn: false,
			Paths: []string{
				filepath.Join(localAppData, "NVIDIA", "DXCache"),
				filepath.Join(localAppData, "NVIDIA", "GLCache"),
			},
		},
		Item{
			App:       "AMD",
			Label:     "shader cache",
			DefaultOn: false,
			Paths: []string{
				filepath.Join(localAppData, "AMD", "DxCache"),
				filepath.Join(localAppData, "AMD", "VkCache"),
				filepath.Join(localAppData, "AMD", "OglCache"),
			},
		},
		Item{
			App:       "Razer Synapse",
			Label:     "cache",
			DefaultOn: true,
			Paths:     chromiumSet(filepath.Join(localAppData, "Razer", "RazerAppEngine", "User Data", "Default")),
		},
	)

	// --- Creative ---

	items = append(items,
		Item{
			App:       "Spotify",
			Label:     "streaming cache",
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
				chromiumSet(filepath.Join(localAppData, "Packages", "SpotifyAB.SpotifyMusic_*", "LocalCache", "Spotify"))...,
			),
		},
		Item{
			App:       "Adobe",
			Label:     "media cache",
			DefaultOn: false,
			Paths: []string{
				filepath.Join(appData, "Adobe", "Common", "Media Cache Files"),
			},
		},
		Item{
			App:       "Blender",
			Label:     "cache",
			DefaultOn: true,
			Globs: []string{
				filepath.Join(appData, "Blender Foundation", "Blender", "*", "cache"),
			},
		},
		Item{
			App:       "Figma",
			Label:     "desktop cache",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(appData, "Figma", "Desktop"),
			},
		},
		Item{
			App:       "Notion",
			Label:     "cache",
			DefaultOn: true,
			Paths:     electronSet(filepath.Join(appData, "Notion")),
		},
		Item{
			App:       "Vortex",
			Label:     "cache",
			DefaultOn: true,
			Paths:     chromiumSet(filepath.Join(appData, "Vortex")),
		},
	)

	// --- System ---

	items = append(items,
		Item{
			App:       "Windows",
			Label:     "Temp folder",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Temp"),
			},
		},
		Item{
			App:       "Windows",
			Label:     "update download cache",
			DefaultOn: true,
			Paths: func() []string {
				if systemRoot == "" {
					return nil
				}
				return []string{filepath.Join(systemRoot, "SoftwareDistribution", "Download")}
			}(),
		},
		Item{
			App:       "Windows",
			Label:     "prefetch",
			DefaultOn: false,
			Paths: func() []string {
				if systemRoot == "" {
					return nil
				}
				return []string{filepath.Join(systemRoot, "Prefetch")}
			}(),
		},
		Item{
			App:       "Windows",
			Label:     "thumbnail + icon cache",
			DefaultOn: true,
			Globs: []string{
				filepath.Join(localAppData, "Microsoft", "Windows", "Explorer", "thumbcache*.db"),
				filepath.Join(localAppData, "Microsoft", "Windows", "Explorer", "iconcache*"),
			},
		},
		Item{
			App:       "Windows Error Reporting",
			Label:     "report archives",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "Microsoft", "Windows", "WER", "ReportArchive"),
				filepath.Join(localAppData, "Microsoft", "Windows", "WER", "ReportQueue"),
			},
		},
		Item{
			App:       "Crash dumps",
			Label:     "local crash dumps",
			DefaultOn: true,
			Paths: []string{
				filepath.Join(localAppData, "CrashDumps"),
			},
		},
		Item{
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
