# Velopack Installer Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remplacer le projet `ImageViewer.Updater` (.NET FX 4.8) et `Helpers/Update.cs` par [Velopack](https://github.com/velopack/velopack) en une seule release majeure (1.0.0), avec auto-update intégré, deltas binaires, rollback automatique, et nettoyage transparent de l'ancien install au premier lancement post-Setup.

**Architecture:** L'app principale embarque le NuGet `Velopack`. `Startup.Main` invoque `VelopackApp.Build().WithFirstRun(LegacyCleanup.Run).Run()` avant `Application.Start`. Un singleton `UpdateManager` vit dans `Context`, lit GitHub Releases via `GithubSource`, et drive le toast existant + le bouton « Vérifier les mises à jour » de `DialogAbout`. Publish `dotnet publish -c Release -r win-x64` self-contained, packing via `vpk pack`, distribution sur GitHub Releases.

**Tech Stack:** .NET 8, WinUI 3, WindowsAppSDK, Velopack (NuGet + dotnet tool global `vpk`), GitHub Releases.

**Spec source:** `docs/superpowers/specs/2026-04-29-velopack-installer-migration-design.md`

**Note tests :** le projet n'a pas de framework de test. Chaque task qui touche du code C# vérifie via `dotnet build` (avec `TreatWarningsAsErrors` actif) et un smoke test manuel quand pertinent. La validation fonctionnelle complète (install neuve, cleanup legacy, auto-update bout en bout) reste l'étape opérateur après merge — détaillée dans la section "Validation manuelle" du spec.

**Environnement de travail :** PowerShell sur Windows. Toutes les commandes shell ci-dessous sont exécutables tel quel dans `pwsh`.

---

## File map

**À créer**
- `ImageViewer/Helpers/LegacyCleanup.cs` — code de migration jetable (~25 lignes), supprimable en une release future une fois tous les utilisateurs migrés.

**À modifier**
- `.gitignore` — ajouter `Releases/` et `publish/`.
- `ImageViewer/ImageViewer.csproj` — `PackageReference Velopack`, `<SelfContained>true</SelfContained>`, `<RuntimeIdentifier>` (singulier), `<PublishSingleFile>false</PublishSingleFile>`, bump `<Version>` à `1.0.0`.
- `ImageViewer/App.xaml.cs` — ajouter `VelopackApp.Build()...Run()` au tout début de `Startup.Main`.
- `ImageViewer/Helpers/Context.cs` — propriété `UpdateMgr`, propriété `PendingUpdate`, réécriture de `CheckUpdate()`.
- `ImageViewer/Helpers/NotificationsManger.cs` — `HandleNotificationAsync` recâblé sur `UpdateMgr.ApplyUpdatesAndRestart`.
- `ImageViewer/Views/DialogAbout.xaml.cs` — recâblage des handlers `ButtonCheckUpdate_Click` et `ButtonDownloadUpdate_Click` + lecture de `Context.PendingUpdate` au lieu de `Update.HasUpdate`.
- `ImageViewer.sln` — retrait du projet `ImageViewer.Updater`.
- `README.md` — section "How to install" et "How to build" actualisées (Setup.exe Velopack + mention warning SmartScreen).

**À supprimer**
- `ImageViewer/Helpers/Update.cs`
- `ImageViewer.Updater/` (dossier complet).

---

## Task 1: Préparer .gitignore et csproj

**Files:**
- Modify: `.gitignore`
- Modify: `ImageViewer/ImageViewer.csproj`

- [ ] **Step 1: Ajouter `Releases/` et `publish/` à `.gitignore`**

Ajouter à la fin du fichier `.gitignore` :

```gitignore
# Velopack output
Releases/
publish/
```

- [ ] **Step 2: Ajouter le NuGet Velopack**

Depuis la racine du repo, en PowerShell :

```pwsh
dotnet add ImageViewer/ImageViewer.csproj package Velopack
```

Cette commande pin la dernière version stable de Velopack dans le csproj.

- [ ] **Step 3: Vérifier que la PackageReference a été ajoutée**

Ouvrir `ImageViewer/ImageViewer.csproj` et vérifier qu'une ligne `<PackageReference Include="Velopack" Version="X.Y.Z" />` apparaît dans le `<ItemGroup>` qui contient les autres packages NuGet (autour de la ligne 62-68).

- [ ] **Step 4: Modifier le `<PropertyGroup>` racine**

Dans `ImageViewer/ImageViewer.csproj`, dans le premier `<PropertyGroup>` (autour des lignes 3-30) :

- **Remplacer** la ligne `<RuntimeIdentifiers>win-x64</RuntimeIdentifiers>` (pluriel) par `<RuntimeIdentifier>win-x64</RuntimeIdentifier>` (singulier).
- **Ajouter** juste après cette ligne :

```xml
<SelfContained>true</SelfContained>
<PublishSingleFile>false</PublishSingleFile>
```

- [ ] **Step 5: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`. Aucun warning (TreatWarningsAsErrors actif). Si erreur, lire le message et corriger avant de continuer.

- [ ] **Step 6: Commit**

```pwsh
git add .gitignore ImageViewer/ImageViewer.csproj
git commit -m "Add Velopack package and switch to self-contained publish"
```

---

## Task 2: Bumper la version applicative à 1.0.0

**Files:**
- Modify: `ImageViewer/ImageViewer.csproj`

- [ ] **Step 1: Modifier `<Version>`**

Dans `ImageViewer/ImageViewer.csproj`, remplacer :

```xml
<Version>0.1.10-beta</Version>
```

par :

```xml
<Version>1.0.0</Version>
```

`<FileVersion>` peut rester à `1.23.5.3` ou être bumpé librement — ce champ Win32 ne sert qu'à l'inspecteur de propriétés Windows.

- [ ] **Step 2: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`.

- [ ] **Step 3: Commit**

```pwsh
git add ImageViewer/ImageViewer.csproj
git commit -m "Bump version to 1.0.0 for Velopack first release"
```

---

## Task 3: Créer Helpers/LegacyCleanup.cs

**Files:**
- Create: `ImageViewer/Helpers/LegacyCleanup.cs`

- [ ] **Step 1: Créer le fichier**

Créer `ImageViewer/Helpers/LegacyCleanup.cs` avec le contenu exact suivant :

```csharp
using System;
using System.Diagnostics;
using System.IO;

namespace ImageViewer.Helpers;

internal static class LegacyCleanup
{
    public static void Run()
    {
        string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string legacy = Path.Combine(localAppData, "Dragon Industries");
        string shortcut = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.StartMenu),
            "Programs", "Image Viewer.lnk");

        TrySwallow(() => { if (Directory.Exists(legacy)) Directory.Delete(legacy, true); });
        TrySwallow(() => { if (File.Exists(shortcut)) File.Delete(shortcut); });
    }

    private static void TrySwallow(Action action)
    {
        try { action(); }
        catch (Exception ex) { Debug.WriteLine($"Legacy cleanup: {ex.Message}"); }
    }
}
```

- [ ] **Step 2: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`. Le fichier est compilé même s'il n'est pas encore référencé.

- [ ] **Step 3: Commit**

```pwsh
git add ImageViewer/Helpers/LegacyCleanup.cs
git commit -m "Add LegacyCleanup helper to remove old Dragon Industries install"
```

---

## Task 4: Hook VelopackApp.Build dans Startup.Main

**Files:**
- Modify: `ImageViewer/App.xaml.cs:14-30`

- [ ] **Step 1: Ajouter le using Velopack**

Dans `ImageViewer/App.xaml.cs`, ajouter dans le bloc des `using` (autour des lignes 1-11) :

```csharp
using Velopack;
```

Garder l'ordre alphabétique des using si le projet le respecte, sinon coller en fin de bloc.

- [ ] **Step 2: Insérer `VelopackApp.Build()` au début de `Main`**

Dans `App.xaml.cs`, modifier la méthode `Main` du `class Startup` (lignes 19-29). Le code actuel est :

```csharp
private static void Main(string[] args)
{
    Context.Instance().LaunchArgs = args;

    global::WinRT.ComWrappersSupport.InitializeComWrappers();
    global::Microsoft.UI.Xaml.Application.Start(p => {
        DispatcherQueueSynchronizationContext context = new global::Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(global::Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread());
        global::System.Threading.SynchronizationContext.SetSynchronizationContext(context);
        _ = new App();
    });
}
```

Le remplacer par :

```csharp
private static void Main(string[] args)
{
    VelopackApp.Build()
        .WithFirstRun(_ => Helpers.LegacyCleanup.Run())
        .Run();

    Context.Instance().LaunchArgs = args;

    global::WinRT.ComWrappersSupport.InitializeComWrappers();
    global::Microsoft.UI.Xaml.Application.Start(p => {
        DispatcherQueueSynchronizationContext context = new global::Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(global::Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread());
        global::System.Threading.SynchronizationContext.SetSynchronizationContext(context);
        _ = new App();
    });
}
```

`VelopackApp.Build().Run()` doit être la **première instruction** de `Main`. Si l'app reçoit un argument CLI Velopack (`--veloapp-install`, `--veloapp-updated`, etc.), `.Run()` exécute le hook puis `Environment.Exit(0)` et la suite de `Main` n'est pas atteinte. Sans args Velopack, `.Run()` retourne immédiatement et le démarrage normal continue.

- [ ] **Step 3: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`.

- [ ] **Step 4: Smoke test — lancer l'app en debug**

Lancer l'app via VS (F5) ou en ligne de commande :

```pwsh
dotnet run --project ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : la fenêtre principale s'ouvre normalement, sans crash. (Velopack `.Run()` retourne immédiatement quand l'app n'est pas lancée par Setup.exe.) Fermer l'app.

- [ ] **Step 5: Commit**

```pwsh
git add ImageViewer/App.xaml.cs
git commit -m "Wire VelopackApp.Build at the very start of Startup.Main"
```

---

## Task 5: Ajouter UpdateMgr et PendingUpdate à Context

**Files:**
- Modify: `ImageViewer/Helpers/Context.cs`

- [ ] **Step 1: Ajouter les usings Velopack**

Dans `ImageViewer/Helpers/Context.cs`, ajouter dans le bloc des `using` (autour des lignes 1-21) :

```csharp
using Velopack;
using Velopack.Sources;
```

- [ ] **Step 2: Ajouter `UpdateMgr`, `PendingUpdate`, et `SetPendingUpdate`**

Dans `Context.cs`, dans le `class Context` (autour des lignes 34-46, là où sont déjà déclarés `_Instance`, `FolderFiles`, `CurrentIndex`, `MemoryOnly`, `LaunchArgs`, `MainWindow`, `NotificationsManger`), ajouter après la déclaration de `NotificationsManger` :

```csharp
public UpdateManager UpdateMgr { get; } = new(
    new GithubSource(
        repoUrl: "https://github.com/dragonofmercy/image-viewer",
        accessToken: null,
        prerelease: false));

public UpdateInfo PendingUpdate { get; private set; }

public void SetPendingUpdate(UpdateInfo info) => PendingUpdate = info;
```

Notes :
- `PendingUpdate { get; private set; }` au lieu d'un champ `_pendingUpdate` séparé — évite le warning CS0649 (field never assigned) qui casserait le build avec `TreatWarningsAsErrors`. Le setter privé permet à `Context.CheckUpdate` (méthode interne à la classe) d'écrire directement `PendingUpdate = ...` en Task 6.
- `SetPendingUpdate` permet aux call sites externes (`DialogAbout` en Task 8) d'écrire l'état partagé après une check manuelle déclenchée par l'utilisateur.
- Le constructeur de `Context` est implicite — l'initialisation inline de `UpdateMgr` se fait au moment de `new Context()` dans `Instance()`. Pas besoin de toucher au constructeur.

- [ ] **Step 3: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`. Aucun warning.

- [ ] **Step 4: Commit**

```pwsh
git add ImageViewer/Helpers/Context.cs
git commit -m "Add UpdateManager, PendingUpdate, and SetPendingUpdate to Context"
```

---

## Task 6: Réécrire Context.CheckUpdate()

**Files:**
- Modify: `ImageViewer/Helpers/Context.cs:493-533`

- [ ] **Step 1: Remplacer le corps de `CheckUpdate()`**

Dans `ImageViewer/Helpers/Context.cs`, localiser la méthode `public async void CheckUpdate()` (autour des lignes 493-533). Le corps actuel est :

```csharp
public async void CheckUpdate()
{
    if (string.IsNullOrEmpty(Settings.UpdateInterval))
    {
        return;
    }

    if (!string.IsNullOrEmpty(Settings.LastUpdateCheck))
    {
        DateTime now = DateTime.Now;
        DateTime lastCheck = DateTime.Parse(Settings.LastUpdateCheck);

        switch (Settings.UpdateInterval)
        {
            case "day":
                lastCheck = lastCheck.AddDays(1);
                break;
            case "week":
                lastCheck = lastCheck.AddDays(7);
                break;
            default:
                lastCheck = lastCheck.AddMonths(1);
                break;
        }

        if (lastCheck.Date > now.Date)
        {
            return;
        }
    }

    if (!await Update.CheckNewVersionAsync()) return;

    AppNotificationBuilder builder = new AppNotificationBuilder()
        .AddText(Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"))
        .AddButton(new AppNotificationButton(Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE")).AddArgument("action", "doUpdate"));

    NotificationsManger.Runtime.Show(builder.BuildNotification());
}
```

Remplacer par :

```csharp
public async void CheckUpdate()
{
    if (string.IsNullOrEmpty(Settings.UpdateInterval))
    {
        return;
    }

    if (!string.IsNullOrEmpty(Settings.LastUpdateCheck))
    {
        DateTime now = DateTime.Now;
        DateTime lastCheck = DateTime.Parse(Settings.LastUpdateCheck);

        switch (Settings.UpdateInterval)
        {
            case "day":
                lastCheck = lastCheck.AddDays(1);
                break;
            case "week":
                lastCheck = lastCheck.AddDays(7);
                break;
            default:
                lastCheck = lastCheck.AddMonths(1);
                break;
        }

        if (lastCheck.Date > now.Date)
        {
            return;
        }
    }

    if (!UpdateMgr.IsInstalled) return;

    try
    {
        PendingUpdate = await UpdateMgr.CheckForUpdatesAsync();
        Settings.LastUpdateCheck = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss");
    }
    catch (Exception ex)
    {
        Debug.WriteLine($"Update check failed: {ex.Message}");
        return;
    }

    if (PendingUpdate == null) return;

    AppNotificationBuilder builder = new AppNotificationBuilder()
        .AddText(Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"))
        .AddButton(new AppNotificationButton(Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE")).AddArgument("action", "doUpdate"));

    NotificationsManger.Runtime.Show(builder.BuildNotification());
}
```

Changements :
- `if (!await Update.CheckNewVersionAsync()) return;` → bloc `try/catch` qui appelle `UpdateMgr.CheckForUpdatesAsync()` et stocke le résultat dans la propriété `PendingUpdate` (setter privé accessible depuis l'intérieur de la classe).
- `Settings.LastUpdateCheck` était settled par `Update.CheckNewVersionAsync` — il est maintenant settled inline ici, **uniquement en cas de succès** (en cas d'erreur réseau on ne touche pas au champ pour retenter au prochain démarrage).
- Skip si `!UpdateMgr.IsInstalled` (mode dev en VS).

- [ ] **Step 2: Vérifier que `Debug` est déjà importé**

`Debug.WriteLine` requiert `using System.Diagnostics;`. Vérifier que cette ligne est déjà présente en haut de `Context.cs` (elle l'est déjà — ligne 5).

- [ ] **Step 3: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`. **Note :** `Helpers/Update.cs` existe encore, donc la référence `Update.CheckNewVersionAsync` y est encore valide à l'extérieur de `Context.cs` — pas de cassure du build à ce stade.

- [ ] **Step 4: Commit**

```pwsh
git add ImageViewer/Helpers/Context.cs
git commit -m "Rewrite Context.CheckUpdate to use Velopack UpdateManager"
```

---

## Task 7: Recâbler NotificationsManger sur UpdateManager

**Files:**
- Modify: `ImageViewer/Helpers/NotificationsManger.cs`

- [ ] **Step 1: Remplacer le corps de `HandleNotificationAsync`**

Dans `ImageViewer/Helpers/NotificationsManger.cs`, localiser la méthode `HandleNotificationAsync` (lignes 32-50). Le corps actuel est :

```csharp
private async Task HandleNotificationAsync(AppNotificationActivatedEventArgs args)
{
    switch (args.Arguments["action"])
    {
        case "doUpdate":
            try
            {
                await Update.ApplyUpdate();
            }
            catch (Exception ex)
            {
                AppNotificationBuilder builder = new AppNotificationBuilder()
                    .AddText(ex.Message);

                Runtime.Show(builder.BuildNotification());
            }
            break;
    }
}
```

Le remplacer par :

```csharp
private async Task HandleNotificationAsync(AppNotificationActivatedEventArgs args)
{
    switch (args.Arguments["action"])
    {
        case "doUpdate":
            UpdateInfo pending = Context.Instance().PendingUpdate;
            if (pending == null) return;

            try
            {
                await Context.Instance().UpdateMgr.DownloadUpdatesAsync(pending);
                Context.Instance().UpdateMgr.ApplyUpdatesAndRestart(pending);
            }
            catch (Exception ex)
            {
                AppNotificationBuilder builder = new AppNotificationBuilder()
                    .AddText(ex.Message);

                Runtime.Show(builder.BuildNotification());
            }
            break;
    }
}
```

- [ ] **Step 2: Ajouter le using Velopack**

En haut de `NotificationsManger.cs`, ajouter :

```csharp
using Velopack;
```

- [ ] **Step 3: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`. `Helpers/Update.cs` existe toujours mais `NotificationsManger.cs` ne le référence plus.

- [ ] **Step 4: Commit**

```pwsh
git add ImageViewer/Helpers/NotificationsManger.cs
git commit -m "Wire NotificationsManger doUpdate action to UpdateManager"
```

---

## Task 8: Réécrire DialogAbout.xaml.cs

**Files:**
- Modify: `ImageViewer/Views/DialogAbout.xaml.cs`

- [ ] **Step 1: Remplacer le contenu complet de `DialogAbout.xaml.cs`**

Le fichier actuel (105 lignes) référence `Update.HasUpdate`, `Update.CheckNewVersionAsync`, `Update.ApplyUpdate`. Tout doit être recâblé sur `Context.UpdateMgr`.

Remplacer le contenu **complet** de `ImageViewer/Views/DialogAbout.xaml.cs` par :

```csharp
using System;
using System.Net.Http;

using ImageViewer.Helpers;
using ImageViewer.Utilities;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

using Velopack;

namespace ImageViewer.Views;

public sealed partial class DialogAbout : Page
{
    private readonly ContentDialog Dialog;

    public DialogAbout(ContentDialog e)
    {
        InitializeComponent();
        Dialog = e;

        UpdateSettingsCard.Label = string.Concat("v", Context.GetProductVersion());
        UpdateSettingsCard.Description = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());

        if(Context.Instance().PendingUpdate != null)
        {
            DisplayUpdateMessage();
        }
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        Dialog.Hide();
    }

    private async void ButtonCheckUpdate_Click(object sender, RoutedEventArgs e)
    {
        UpdateStatusInfo.IsOpen = false;
        UpdateCheckingProgress.IsActive = true;
        ButtonCheckUpdate.Visibility = Visibility.Collapsed;
        UpdateCheckingText.Visibility = Visibility.Visible;
        ButtonDownloadUpdate.Visibility = Visibility.Collapsed;

        try
        {
            UpdateInfo info = await Context.Instance().UpdateMgr.CheckForUpdatesAsync();
            Settings.LastUpdateCheck = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss");

            if(info != null)
            {
                Context.Instance().SetPendingUpdate(info);
                DisplayUpdateMessage();
            }
            else
            {
                Context.Instance().SetPendingUpdate(null);
                UpdateStatusInfo.Severity = InfoBarSeverity.Success;
                UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_LATEST");
                UpdateStatusInfo.IsOpen = true;
            }

            UpdateSettingsCard.Description = string.Concat(Culture.GetString("ABOUT_LABEL_LAST_UPDATE"), Settings.LastUpdateCheck.ToUpdateDate());
        }
        catch(HttpRequestException)
        {
            UpdateStatusInfo.Severity = InfoBarSeverity.Error;
            UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_ERROR_NO_INTERNET");
            UpdateStatusInfo.IsOpen = true;
        }

        UpdateCheckingProgress.IsActive = false;
        ButtonCheckUpdate.Visibility = Visibility.Visible;
        UpdateCheckingText.Visibility = Visibility.Collapsed;
    }

    private void DisplayUpdateMessage()
    {
        UpdateStatusInfo.Severity = InfoBarSeverity.Warning;
        UpdateStatusInfo.Title = Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE");
        UpdateStatusInfo.IsOpen = true;

        ButtonDownloadUpdate.Visibility = Visibility.Visible;
    }

    private async void ButtonDownloadUpdate_Click(object sender, RoutedEventArgs e)
    {
        ButtonDownloadUpdate.IsEnabled = false;
        ButtonDownloadUpdate.Content = Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE_DOWNLOADING");

        UpdateInfo pending = Context.Instance().PendingUpdate;
        if(pending == null) return;

        try
        {
            await Context.Instance().UpdateMgr.DownloadUpdatesAsync(pending);
            Context.Instance().UpdateMgr.ApplyUpdatesAndRestart(pending);
        }
        catch(Exception ex)
        {
            UpdateStatusInfo.Severity = InfoBarSeverity.Error;
            UpdateStatusInfo.Title = ex.Message;
            UpdateStatusInfo.IsOpen = true;

            ButtonDownloadUpdate.IsEnabled = true;
            ButtonDownloadUpdate.Content = Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE_RETRY");
        }
    }
}
```

Changements clés :
- `using System.Collections.Generic;` retiré (plus utilisé : on n'attrape plus `KeyNotFoundException`).
- Ajout `using Velopack;`.
- `Update.HasUpdate` → `Context.Instance().PendingUpdate != null`.
- `Update.CheckNewVersionAsync()` → `Context.Instance().UpdateMgr.CheckForUpdatesAsync()` + persistance manuelle de `LastUpdateCheck`.
- `Update.ApplyUpdate()` → `UpdateMgr.DownloadUpdatesAsync(pending)` + `UpdateMgr.ApplyUpdatesAndRestart(pending)`.
- `catch(KeyNotFoundException)` retiré (l'ancien code parsait du JSON GitHub à la main, plus pertinent avec Velopack).
- `Context.Instance().SetPendingUpdate(...)` permet à ce dialog de mettre à jour l'état partagé après une check manuelle. **Cette méthode doit être ajoutée à Context** — voir Step 2 ci-dessous.

- [ ] **Step 2: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`. La méthode `Context.SetPendingUpdate` requise par ce dialog a été ajoutée en Task 5.

- [ ] **Step 3: Commit**

```pwsh
git add ImageViewer/Views/DialogAbout.xaml.cs
git commit -m "Rewire DialogAbout update buttons to UpdateManager"
```

---

## Task 9: Supprimer Helpers/Update.cs

**Files:**
- Delete: `ImageViewer/Helpers/Update.cs`

- [ ] **Step 1: Vérifier qu'aucune référence à `Update` (la classe) ne subsiste hors de `Update.cs`**

```pwsh
Select-String -Path ImageViewer\**\*.cs -Pattern '\bUpdate\.' -CaseSensitive
```

Attendu : seules les occurrences dans `ImageViewer\Helpers\Update.cs` lui-même apparaissent. Tous les call sites externes (`Context.cs`, `NotificationsManger.cs`, `DialogAbout.xaml.cs`) ont été nettoyés en Tasks 6/7/8.

Si une autre référence apparaît (par exemple un commentaire `/// Update file infos.` dans `Context.cs` ligne 211 — c'est un commentaire, pas une référence à la classe), l'ignorer. Seules les références au type `Update` ou ses membres (`Update.X`, `await Update.Y()`) doivent être absentes.

- [ ] **Step 2: Supprimer le fichier (et stage la suppression d'un coup)**

```pwsh
git rm ImageViewer/Helpers/Update.cs
```

- [ ] **Step 3: Vérifier que le build Debug passe**

```pwsh
dotnet build ImageViewer/ImageViewer.csproj -c Debug
```

Attendu : `Build succeeded`. Si ça casse, c'est qu'une référence à `Update.X` traîne quelque part — la corriger.

- [ ] **Step 4: Commit**

```pwsh
git commit -m "Remove obsolete Helpers/Update.cs (replaced by Velopack UpdateManager)"
```

---

## Task 10: Retirer le projet ImageViewer.Updater de la solution et supprimer le dossier

**Files:**
- Modify: `ImageViewer.sln`
- Delete: `ImageViewer.Updater/` (dossier complet)

- [ ] **Step 1: Modifier `ImageViewer.sln`**

Le fichier actuel contient (lignes 8-9) :

```
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "ImageViewer.Updater", "ImageViewer.Updater\ImageViewer.Updater.csproj", "{548183AB-FD58-43C1-9FD4-2B0A9C601D33}"
EndProject
```

Supprimer ces deux lignes.

Et dans `GlobalSection(ProjectConfigurationPlatforms)` (autour des lignes 21-24) :

```
{548183AB-FD58-43C1-9FD4-2B0A9C601D33}.Debug|x64.ActiveCfg = Debug|x64
{548183AB-FD58-43C1-9FD4-2B0A9C601D33}.Debug|x64.Build.0 = Debug|x64
{548183AB-FD58-43C1-9FD4-2B0A9C601D33}.Release|x64.ActiveCfg = Release|x64
{548183AB-FD58-43C1-9FD4-2B0A9C601D33}.Release|x64.Build.0 = Release|x64
```

Supprimer ces 4 lignes (toutes les entrées avec le GUID `548183AB-FD58-43C1-9FD4-2B0A9C601D33`).

- [ ] **Step 2: Supprimer le dossier `ImageViewer.Updater/` (et stage la suppression d'un coup)**

```pwsh
git rm -r ImageViewer.Updater
```

- [ ] **Step 3: Vérifier que la solution build encore (Debug et Release)**

```pwsh
dotnet build ImageViewer.sln -c Debug
dotnet build ImageViewer.sln -c Release
```

Attendu : `Build succeeded` pour les deux. Le solution file ne contient plus que le projet `ImageViewer`.

- [ ] **Step 4: Commit**

```pwsh
git add ImageViewer.sln
git commit -m "Remove ImageViewer.Updater project (replaced by Velopack Setup.exe)"
```

---

## Task 11: Mettre à jour README.md

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Remplacer la section "How to install"**

Dans `README.md`, la section actuelle (lignes 25-31) est :

```markdown
## How to install

1. Download ImageViewer.Updater.exe from the last release  
https://github.com/dragonofmercy/image-viewer/releases
2. Trust the file (Help needed for that, just contact me)
3. Run ImageViewer.Updater.exe
4. Done
```

La remplacer par :

```markdown
## How to install

1. Download `Setup.exe` from the latest release  
https://github.com/dragonofmercy/image-viewer/releases
2. Run `Setup.exe`. Windows SmartScreen will warn that the file is not signed — click "More info" then "Run anyway".
3. The app installs to `%LOCALAPPDATA%\Dragon.ImageViewer\` and adds a Start Menu shortcut.
4. Updates are downloaded and applied automatically from within the app.
```

- [ ] **Step 2: Optionnel — actualiser la section "How to build"**

La section build actuelle (lignes 11-23) reste valide (VS 2022 + Windows App SDK). On peut **ajouter** une note à la fin pour décrire le packaging Velopack :

```markdown
## How to release (maintainer only)

```bash
dotnet tool install -g vpk   # one-time install
vpk download github --repoUrl https://github.com/dragonofmercy/image-viewer
dotnet publish ImageViewer/ImageViewer.csproj -c Release -r win-x64 -o publish
vpk pack --packId Dragon.ImageViewer --packTitle "Image Viewer" --packAuthors "DragonOfMercy" --packVersion <X.Y.Z> --packDir publish --mainExe ImageViewer.exe --icon ImageViewer/ImageViewer.ico
```

Then upload the contents of `Releases/` to a new GitHub Release.
```

(L'imbrication des triple-backticks dans un fichier markdown qui contient ce plan se fait avec quatre backticks — voir le rendu final dans le plan ci-dessus.)

- [ ] **Step 3: Commit**

```pwsh
git add README.md
git commit -m "Update README install steps for Velopack Setup.exe"
```

---

## Task 12: Smoke test final

**Files:** *(aucun changement de code)*

- [ ] **Step 1: Build complet en Release**

```pwsh
dotnet build ImageViewer.sln -c Release
```

Attendu : `Build succeeded`. Aucun warning. Pas d'erreur dans le PostBuild target qui prune les `.mui`.

- [ ] **Step 2: Publish self-contained pour valider la pipeline**

```pwsh
dotnet publish ImageViewer/ImageViewer.csproj -c Release -r win-x64 -o publish
```

Attendu : le dossier `publish/` est créé et contient :
- `ImageViewer.exe` (la version Release renommée)
- Un grand nombre de DLLs .NET (`System.*.dll`, `Microsoft.WindowsAppRuntime.*.dll`)
- Sous-dossiers `en-us/`, `fr-FR/`, `fr/` (les `.mui` localisés)
- Aucun `.pdb` autre que `ImageViewer.pdb`, aucun `.xml` autre que `ImageViewer.xml` (PostBuild target)

Vérifier la taille du dossier (~80-100 Mo attendu).

- [ ] **Step 3: Lancer le binaire publié**

```pwsh
.\publish\ImageViewer.exe
```

Attendu : la fenêtre principale s'ouvre. Ouvrir une image, vérifier que rotation et crop fonctionnent. Fermer.

- [ ] **Step 4: Vérifier `git status` et logger l'historique**

```pwsh
git status
git log --oneline -15
```

Attendu : working tree clean, et 11 commits "Velopack" ajoutés depuis la pointe d'origine.

- [ ] **Step 5: Pas de commit**

Cette task est purement vérificatoire. Aucun fichier modifié à committer.

---

## Hors scope du plan d'implémentation (responsabilité opérateur)

Une fois ce plan exécuté et mergé sur `main`, l'opérateur (utilisateur) effectue **manuellement** :

1. **Validation manuelle complète** (checklist Section "Validation manuelle avant publication" du spec) :
   - Install neuve sur VM Windows 11 vierge.
   - Cleanup legacy sur VM avec ancien install.
   - Auto-update bout en bout entre 1.0.0 et 1.0.1 (release fictive).
   - Rollback automatique sur release intentionnellement cassée.

2. **Première release Velopack publique** :
   ```pwsh
   dotnet tool install -g vpk
   # vpk download github n'a rien à récupérer pour la première release Velopack
   dotnet publish ImageViewer/ImageViewer.csproj -c Release -r win-x64 -o publish
   vpk pack --packId Dragon.ImageViewer --packTitle "Image Viewer" --packAuthors "DragonOfMercy" --packVersion 1.0.0 --packDir publish --mainExe ImageViewer.exe --icon ImageViewer/ImageViewer.ico
   ```
   Puis upload manuel des fichiers de `Releases/` sur GitHub Releases via l'UI ou via `vpk upload github`.

3. **Annonce dans les release notes** que les utilisateurs existants doivent télécharger le nouveau `Setup.exe` manuellement (cleanup automatique de l'ancien install au premier lancement).
