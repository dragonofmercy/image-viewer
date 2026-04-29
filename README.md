# Image Viewer for Windows

[![Donate](https://img.shields.io/badge/Donate-PayPal-green.svg)](https://ko-fi.com/dragonofmercy)
[![Twitter](https://img.shields.io/twitter/url/http/shields.io.svg?style=flat&logo=twitter)](https://twitter.com/intent/tweet?text=ImageViewer.%20A%20new%20image%20viewer%20for%20Windows%2010%20%26%2011.%20%23imageviewer%20%23dotnet%20%23winui3%20via%20%40dragonofmercy&url=https%3A%2F%2Fgithub.com%2Fdragonofmercy%2Fimage-viewer)

Simple image viewer for Windows.  
Image extensions supported: jpg, jpeg, bmp, png, gif, tif, tiff, tga, ico, webp and svg

![](/documentation/assets/screen1.jpg)

## How to build

**Requirements**: Windows 10 22H2 / Windows 11, .NET 8 SDK, and the Windows App SDK build tools.

### From Visual Studio (easiest)

1. Install [Visual Studio 2022](https://visualstudio.microsoft.com/vs/) (Community edition is fine).
2. During install, enable the **Windows application development** workload, which pulls the .NET 8 SDK and the Windows App SDK build tools ([Microsoft docs](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/set-up-your-development-environment)).

   ![](/documentation/assets/vs2022_install_req.jpg)

3. Open `ImageViewer.sln`, wait for NuGet restore, then build (`Ctrl+Shift+B`) or run (`F5`).

### From the command line

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

## Credits
ImageSharp by SixLabors: https://github.com/SixLabors/ImageSharp

## 

If this project help to increase your productivity, you can give me a cup of coffee :) 

[![Donate](https://cdn.ko-fi.com/cdn/kofi2.png?v=3)](https://ko-fi.com/dragonofmercy)

## How to release (maintainer only)

One-time setup: `dotnet tool install -g vpk`

Bump `<Version>` in `ImageViewer\ImageViewer.csproj` first, then from the repo root:

```pwsh
.\Build-Release.ps1
```

The helper script reads the version from the csproj (so it always matches the binary), then wraps `dotnet publish` (self-contained, `win-x64`) and `vpk pack` with the standard arguments (`--packId Dragon.ImageViewer`, `--shortcuts StartMenuRoot`, etc.). Output goes to `Releases\`.

For deltas across releases, run `vpk download github --repoUrl https://github.com/dragonofmercy/image-viewer` before the script so the previous `.nupkg` is available.

Then upload the contents of `Releases\` to a new GitHub Release.
