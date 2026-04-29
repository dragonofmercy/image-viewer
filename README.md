# Image Viewer for Windows

[![Donate](https://img.shields.io/badge/Donate-PayPal-green.svg)](https://ko-fi.com/dragonofmercy)
[![Twitter](https://img.shields.io/twitter/url/http/shields.io.svg?style=flat&logo=twitter)](https://twitter.com/intent/tweet?text=ImageViewer.%20A%20new%20image%20viewer%20for%20Windows%2010%20%26%2011.%20%23imageviewer%20%23dotnet%20%23winui3%20via%20%40dragonofmercy&url=https%3A%2F%2Fgithub.com%2Fdragonofmercy%2Fimage-viewer)

Simple image viewer for Windows.  
Image extensions supported: jpg, jpeg, bmp, png, gif, tif, tiff, tga, ico, webp and svg

![](/documentation/assets/screen1.jpg)

## How to build

1. Install Visual Studio 2022 Community  
https://visualstudio.microsoft.com/vs/

2. Install Tools for Windows App SDK  
https://docs.microsoft.com/en-us/windows/apps/windows-app-sdk/set-up-your-development-environment

![](/documentation/assets/vs2022_install_req.jpg)

3. Open the solution
4. Wait for nuget requirements to download
5. Build the project

## How to install

1. Download `Setup.exe` from the latest release  
https://github.com/dragonofmercy/image-viewer/releases
2. Run `Setup.exe`. Windows SmartScreen will warn that the file is not signed - click "More info" then "Run anyway".
3. The app installs to `%LOCALAPPDATA%\Dragon.ImageViewer\` and adds a Start Menu shortcut.
4. Updates are downloaded and applied automatically from within the app.

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
