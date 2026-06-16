<p align="center">
  <img src="https://raw.githubusercontent.com/dragonofmercy/image-viewer/main/.github/og-image.png" alt="Image Viewer - fast native image viewer for Windows" width="100%">
</p>

# Image Viewer for Windows

[![Latest release](https://badgen.net/github/release/dragonofmercy/image-viewer/stable?icon=github)](https://github.com/dragonofmercy/image-viewer/releases/latest)
[![Downloads](https://badgen.net/github/assets-dl/dragonofmercy/image-viewer?icon=github)](https://github.com/dragonofmercy/image-viewer/releases)
[![License](https://badgen.net/github/license/dragonofmercy/image-viewer)](LICENSE)
[![.NET](https://img.shields.io/badge/.NET-10-512BD4?logo=dotnet&logoColor=white)](https://dotnet.microsoft.com/)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D6?logo=windows&logoColor=white)](https://www.microsoft.com/windows)
[![Donate](https://img.shields.io/badge/Donate-Ko--fi-FF5E5B?logo=kofi&logoColor=white)](https://ko-fi.com/dragonofmercy)

Simple image viewer for Windows.  
Image extensions supported: jpg, jpeg, bmp, png, gif, tif, tiff, tga, ico, webp and svg

![](/.github/screen.webp)

## How to build

**Requirements**: Windows 10 22H2 / Windows 11, [.NET 10 SDK](https://dotnet.microsoft.com/download), and the Windows App SDK build tools ([Microsoft docs](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/set-up-your-development-environment)).

```pwsh
dotnet build ImageViewer.sln -c Release
```

For a self-contained, runnable folder that needs no .NET runtime installed on the target machine:

```pwsh
dotnet publish ImageViewer\ImageViewer.csproj -c Release -r win-x64 -o publish
```

## How to install

**Requirements**: Windows 10 22H2 or Windows 11 (x64). The app is self-contained, no .NET or Windows App SDK runtime install needed.

1. Download `Setup.exe` from the [latest release](https://github.com/dragonofmercy/image-viewer/releases).
2. Run `Setup.exe`. Windows SmartScreen will warn that the file is not signed - click **More info** then **Run anyway**.
3. The installer is silent (no wizard). It places the app in `%LOCALAPPDATA%\Dragon.ImageViewer\`, adds a Start Menu shortcut, and launches the app when done.
4. Updates are downloaded and applied automatically from within the app (toast notification when a new version is available, plus a **Check for updates** button in the About dialog).

To uninstall, use **Settings → Apps & features → Image Viewer → Uninstall**.

## How to release (maintainer only)

One-time setup: `dotnet tool install -g vpk`

Bump `<Version>` in `ImageViewer\ImageViewer.csproj` first, then from the repo root:

```pwsh
.\Build-Release.ps1
```

The helper script reads the version from the csproj (so it always matches the binary), then wraps `dotnet publish` (self-contained, `win-x64`) and `vpk pack` with the standard arguments (`--packId Dragon.ImageViewer`, `--shortcuts StartMenuRoot`, etc.). Output goes to `Releases\`.

For deltas across releases, run `vpk download github --repoUrl https://github.com/dragonofmercy/image-viewer` before the script so the previous `.nupkg` is available.

Then upload the contents of `Releases\` to a new GitHub Release.

## Credits

This project stands on the shoulders of these open-source libraries:

- [ImageSharp](https://github.com/SixLabors/ImageSharp) by SixLabors - image decoding, encoding and processing
- [Windows Community Toolkit](https://github.com/CommunityToolkit/Windows) - the `ImageCropper` control
- [SVG](https://github.com/svg-net/SVG) - SVG rasterization
- [Velopack](https://github.com/velopack/velopack) - installer and auto-updates
- [WinUIEx](https://github.com/dotMorten/WinUIEx) - WinUI window helpers

## Support

If this project helps to increase your productivity, you can give me a cup of coffee :)

<a href="https://ko-fi.com/dragonofmercy"><img src="https://cdn.ko-fi.com/cdn/kofi2.png?v=3" alt="Donate" width="160"></a>
