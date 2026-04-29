# Migration de l'installeur custom vers Velopack

**Date** : 2026-04-29
**Statut** : Spec validé, prêt pour planification d'implémentation

## Contexte

Le projet `ImageViewer` ship aujourd'hui avec deux composants :
- `ImageViewer/` — l'app WinUI 3 / .NET 8 elle-même (unpackaged, x64).
- `ImageViewer.Updater/` — un installeur/updater custom en .NET Framework 4.8 WinForms qui télécharge les runtimes (.NET 8 Desktop + WindowsAppRuntime), extrait le ZIP de release dans `%LOCALAPPDATA%\Dragon Industries\` (basé sur `CompanyName` lu via `FileVersionInfo` de l'updater) et crée un raccourci Start Menu `Image Viewer.lnk`.

Cette architecture pose plusieurs problèmes :
- **Drift de versions** : 4 constantes hardcodées dans `ImageViewer.Updater/MainWindow.cs` (versions WindowsAppRuntime + .NET Desktop Runtime) qui doivent rester synchronisées avec `ImageViewer.csproj` à chaque bump. Manuel, oubliable.
- **Maintenance d'un second projet** sur un runtime obsolète (.NET FX 4.8) juste pour un installeur.
- **Logique d'update in-app fragile** : `Helpers/Update.cs` (~220 lignes) implémente une comparaison SemVer maison qui compare les suffixes de pré-release lexicographiquement (`alpha` vs `beta` mal ordonnés) et fetch directement la GitHub Releases API.
- **Pas de deltas** : chaque MAJ retélécharge le `ImageViewer.Updater.exe` complet.
- **Pas de rollback** : si une release est cassée, les users sont bloqués.

L'objectif est de remplacer ces deux composants par [Velopack](https://github.com/velopack/velopack) — un framework moderne d'installeur + auto-update pour applications .NET desktop.

## Décisions de cadrage

| # | Décision | Choix |
|---|----------|-------|
| 1 | Scope du remplacement | **B** — Velopack remplace l'installeur initial **et** l'auto-update in-app |
| 2 | Migration des users existants | **C** — Coupure nette avec nettoyage de l'ancien install par le `Setup.exe` Velopack |
| 3 | Bundling du runtime | **A** — Self-contained complet (.NET + WindowsAppSDK bundlés) |
| 4 | Signature de code | **A** — Pas de signature au début (assumé via README, migration possible vers Azure Trusted Signing plus tard) |
| 5 | Refactor de `Helpers/Update.cs` | **2** — Suppression complète, intégration directe de `UpdateManager` |
| 6 | Localisation du cleanup legacy | Fichier dédié `ImageViewer/Helpers/LegacyCleanup.cs` (jetable proprement après quelques releases) |
| 7 | AppId Velopack | `Dragon.ImageViewer` (figé, ne plus jamais changer après la première release) |
| 8 | Première version Velopack | `1.0.0` (saut depuis `0.1.10-beta` pour marquer la refonte de l'installeur) |

## Ce qui disparaît

- Projet `ImageViewer.Updater/` entier (dossier + entrées dans `ImageViewer.sln`).
- Fichier `ImageViewer/Helpers/Update.cs` (~220 lignes : fetch GitHub, comparaison SemVer maison, download du `.exe` updater).
- Les 4 constantes hardcodées de runtime dans l'updater.

## Ce qui apparaît

- Référence NuGet `Velopack` dans `ImageViewer.csproj` (dernière version stable au moment du commit, à fixer pendant l'implémentation).
- Champ `Context.UpdateMgr` (instance `Velopack.UpdateManager` typée).
- Champ privé `Context._pendingUpdate` (de type `Velopack.UpdateInfo`) pour transporter l'update détectée entre `CheckUpdate()` et `NotificationsManger.HandleNotificationAsync`.
- Nouveau fichier `ImageViewer/Helpers/LegacyCleanup.cs` exposant `LegacyCleanup.Run()` (statique).
- Hook `VelopackApp.Build().WithFirstRun(_ => LegacyCleanup.Run()).Run()` au début de `Startup.Main`.
- Propriétés csproj : `<SelfContained>true</SelfContained>`, `<RuntimeIdentifier>win-x64</RuntimeIdentifier>` (singulier), `<PublishSingleFile>false</PublishSingleFile>`.

## Ce qui reste inchangé

- Toute la logique métier du viewer (`Wrapper.Image`, `Context`, controls, views).
- `Helpers.Settings` et la registry `HKEY_CURRENT_USER\SOFTWARE\Dragon Industries\Image Viewer`. Velopack ne touche jamais à la registry user, donc les settings (theme, taille fenêtre, langue, intervalle de check) sont préservées sans aucune migration.
- Les deux clés de settings `UpdateInterval` (`day` / `week` / `month` / `""`) et `LastUpdateCheck` — c'est l'app qui décide quand appeler `UpdateMgr.CheckForUpdatesAsync()`, exactement comme avant.
- Le toast `AppNotificationManager` qui annonce une MAJ disponible (action `doUpdate`), recâblé pour pointer vers `UpdateManager` au lieu de l'ancien Updater.
- Le bouton « Check for update » dans `DialogAbout`, recâblé pareil.
- Le `PostBuild` target qui prune les `.mui` non-EN/FR — il s'applique pendant `dotnet publish` et garantit que le dossier passé à `vpk pack` est déjà propre.

## Architecture cible

### Initialisation — `Startup.Main` (App.xaml.cs)

L'ordre est critique. Velopack utilise des arguments CLI spéciaux (`--veloapp-install`, `--veloapp-updated`, `--veloapp-obsolete`, `--veloapp-uninstall`) que `Setup.exe` injecte au moment de l'install/update/uninstall. Si l'app les reçoit sans `VelopackApp.Build().Run()` en premier, elle plante avec des args inconnus.

```csharp
private static void Main(string[] args)
{
    VelopackApp.Build()
        .WithFirstRun(_ => LegacyCleanup.Run())
        .Run();
    // Si args contient un veloapp-*, .Run() exécute le hook puis Environment.Exit.
    // Sinon return immédiat et on continue le démarrage normal.

    Context.Instance().LaunchArgs = args;
    WinRT.ComWrappersSupport.InitializeComWrappers();
    Application.Start(p =>
    {
        var ctx = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
        SynchronizationContext.SetSynchronizationContext(ctx);
        _ = new App();
    });
}
```

### Singleton `UpdateManager` dans `Context`

```csharp
internal class Context
{
    public UpdateManager UpdateMgr { get; } = new(
        new GithubSource(
            repoUrl: "https://github.com/dragonofmercy/image-viewer",
            accessToken: null,
            prerelease: false));

    private UpdateInfo _pendingUpdate;
    public UpdateInfo PendingUpdate => _pendingUpdate;

    // ... reste de la classe inchangé
}
```

`UpdateManager` est cheap à construire mais maintient l'état de la dernière check. Le tenir en singleton dans `Context` garantit qu'il est partagé entre `CheckUpdate()` et `NotificationsManger.HandleNotificationAsync`.

Le flag `prerelease: false` peut basculer à `true` plus tard pour exposer un canal beta séparé. Hors scope pour cette migration.

### Flow `Context.CheckUpdate` (réécrit)

```csharp
public async void CheckUpdate()
{
    if (string.IsNullOrEmpty(Settings.UpdateInterval)) return;
    if (!IsCheckIntervalElapsed()) return;
    if (!UpdateMgr.IsInstalled) return;   // skip pendant le dev en VS

    try
    {
        _pendingUpdate = await UpdateMgr.CheckForUpdatesAsync();
        Settings.LastUpdateCheck = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss");
    }
    catch (Exception ex)
    {
        Debug.WriteLine($"Update check failed: {ex.Message}");
        return;   // pas de toast d'erreur, retry au prochain cycle
    }

    if (_pendingUpdate == null) return;

    AppNotificationBuilder builder = new AppNotificationBuilder()
        .AddText(Culture.GetString("ABOUT_UPDATE_INFO_UPDATE_AVAILABLE"))
        .AddButton(new AppNotificationButton(Culture.GetString("ABOUT_BTN_DOWNLOAD_UPDATE"))
            .AddArgument("action", "doUpdate"));

    NotificationsManger.Runtime.Show(builder.BuildNotification());
}
```

`IsCheckIntervalElapsed()` est une méthode privée qui contient la logique actuelle de `Update.CheckUpdate` autour de `Settings.LastUpdateCheck` + intervalle (`day` / `week` / `month`).

### Flow `doUpdate` dans `NotificationsManger`

```csharp
case "doUpdate":
    var pending = Context.Instance().PendingUpdate;
    if (pending == null) return;

    try
    {
        await Context.Instance().UpdateMgr.DownloadUpdatesAsync(pending);
        Context.Instance().UpdateMgr.ApplyUpdatesAndRestart(pending);
        // ApplyUpdatesAndRestart kill ce process et relance la nouvelle version.
    }
    catch (Exception ex)
    {
        AppNotificationBuilder builder = new AppNotificationBuilder().AddText(ex.Message);
        Runtime.Show(builder.BuildNotification());
    }
    break;
```

### Cleanup ancien install — `Helpers/LegacyCleanup.cs`

Fichier dédié, isolé pour pouvoir être supprimé proprement (`git rm`) une fois que tous les utilisateurs actifs auront migré (typiquement 2-3 releases plus tard). Appelé une seule fois par Velopack via `WithFirstRun` après le tout premier lancement post-install Velopack, sur les machines qui avaient l'ancien install.

```csharp
namespace ImageViewer.Helpers;

internal static class LegacyCleanup
{
    public static void Run()
    {
        var localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var legacy = Path.Combine(localAppData, "Dragon Industries");
        var shortcut = Path.Combine(
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

L'ancien updater pose les fichiers de l'app directement dans `%LOCALAPPDATA%\Dragon Industries\` (sans sous-dossier `Image Viewer`), donc on supprime ce dossier entier. Le raccourci s'appelle exactement `Image Viewer.lnk`. Tout est best-effort — Velopack créera son propre raccourci au moment de l'install (nom dérivé de `--packTitle`), donc l'utilisateur ne perd pas son point d'entrée même si le delete échoue.

## Modifications du csproj

```xml
<PropertyGroup>
    <!-- existant -->
    <OutputType>WinExe</OutputType>
    <TargetFramework>net8.0-windows10.0.22621</TargetFramework>
    <UseWinUI>true</UseWinUI>
    <WindowsPackageType>None</WindowsPackageType>
    <WindowsAppSDKSelfContained>true</WindowsAppSDKSelfContained>

    <!-- nouveaux / modifiés -->
    <SelfContained>true</SelfContained>
    <RuntimeIdentifier>win-x64</RuntimeIdentifier>     <!-- singulier, remplace RuntimeIdentifiers -->
    <PublishSingleFile>false</PublishSingleFile>       <!-- explicite : WinAppSDK incompatible avec SingleFile -->

    <!-- existant, conservé -->
    <StartupObject>ImageViewer.Startup</StartupObject>
    <defineconstants>DISABLE_XAML_GENERATED_MAIN</defineconstants>
    <!-- ... -->
</PropertyGroup>

<ItemGroup>
    <PackageReference Include="Velopack" Version="0.0.x" />   <!-- pin à la dernière stable au moment du commit -->
    <!-- ... autres packages inchangés -->
</ItemGroup>
```

Le target `PostBuild` qui prune les `.mui` non-EN/FR est conservé tel quel.

## Suppression du projet `ImageViewer.Updater`

Modifier `ImageViewer.sln` pour retirer :
- Le bloc `Project("...") = "ImageViewer.Updater" ... EndProject`.
- Les 4 entrées correspondantes dans `GlobalSection(ProjectConfigurationPlatforms)` (Debug|x64 et Release|x64, ActiveCfg + Build.0).

Supprimer ensuite le dossier `ImageViewer.Updater/` du repo.

## Workflow de release

**Outils requis** :
- Visual Studio 2022 + workload Windows App SDK (déjà installé pour le dev).
- Velopack CLI : `dotnet tool install -g vpk` (à faire une fois par machine).

**Étapes pour publier une nouvelle version** (à exécuter par l'utilisateur depuis la racine du repo) :

```bash
# 1. Bump <Version> dans ImageViewer/ImageViewer.csproj
#    Première release Velopack : 0.1.10-beta -> 1.0.0
#    Le <FileVersion> Win32 peut être bumpé en parallèle ou laissé tel quel.

# 2. Récupérer les .nupkg de la release précédente pour générer un delta binaire
#    (à partir de la release suivant la première Velopack — pas applicable au passage 1.0.0 lui-même)
vpk download github --repoUrl https://github.com/dragonofmercy/image-viewer

# 3. Publish self-contained
dotnet publish ImageViewer/ImageViewer.csproj -c Release -r win-x64 -o publish

# 4. Packer via Velopack
vpk pack \
    --packId Dragon.ImageViewer \
    --packTitle "Image Viewer" \
    --packAuthors "DragonOfMercy" \
    --packVersion 1.0.0 \
    --packDir publish \
    --mainExe ImageViewer.exe \
    --icon ImageViewer/ImageViewer.ico

# 5. Upload manuel des fichiers de Releases/ sur GitHub Releases via l'UI,
#    OU :
# vpk upload github --repoUrl https://github.com/dragonofmercy/image-viewer --releaseName "v1.0.0"
#    (à lancer manuellement, jamais automatisé par Claude — instruction globale "git local only")
```

**AppId Velopack** : `Dragon.ImageViewer`. **Critique** : ne plus jamais changer après la première release Velopack — un changement d'AppId ferait que les users actuels ne reçoivent plus les MAJ (Velopack utilise l'AppId comme clé d'identification). L'install destination sera `%LOCALAPPDATA%\Dragon.ImageViewer\`.

**Sortie de `vpk pack`** dans `Releases/` :
- `Setup.exe` — bootstrapper que les nouveaux users téléchargent depuis GitHub.
- `Dragon.ImageViewer-X.Y.Z-full.nupkg` — package complet de la version.
- `Dragon.ImageViewer-X.Y.Z-delta.nupkg` — diff binaire depuis la version précédente (si présente dans `Releases/` après l'étape 2). Les users existants téléchargent uniquement ce delta lors de l'auto-update.
- `releases.win.json` — manifest que `UpdateManager` lit pour décider quelles MAJ sont disponibles.

Les 4 fichiers (ou 3 à la première release Velopack, sans delta) doivent tous être uploadés sur la GitHub Release pour que `UpdateManager` côté users les trouve via `GithubSource`.

**Le dossier `Releases/`** doit être ajouté au `.gitignore`.

## Gestion d'erreurs

| Cas | Comportement |
|-----|--------------|
| App pas installée (mode dev VS) | `UpdateMgr.IsInstalled == false` → `CheckUpdate()` skip silencieusement. F5 dans VS fonctionne sans erreur. |
| Pas de réseau / GitHub API down | `CheckForUpdatesAsync()` throw → `catch`, `Debug.WriteLine`, pas de mise à jour de `LastUpdateCheck` → retry au prochain démarrage. Pas de toast d'erreur. |
| `DownloadUpdatesAsync` échoue (interrupted, disque plein) | Toast d'erreur localisé. `_pendingUpdate` reste défini, donc un re-click sur le toast retente. Velopack supporte le resume des deltas. |
| `ApplyUpdatesAndRestart` échoue (fichiers lockés) | Velopack marque l'update comme "à appliquer au prochain démarrage" et fait le swap quand les fichiers ne sont plus lockés. Aucun code custom à écrire. |
| Première release Velopack | `vpk download github` ne trouvera pas de release Velopack précédente (les releases actuelles contiennent un `.exe` updater, pas de `.nupkg`). `vpk pack` produit alors un full sans delta — comportement normal pour la transition. |
| Release cassée (crash au lancement) | Velopack détecte le crash au premier run de la nouvelle version et revient automatiquement à la version précédente. Aucun code custom à écrire. |
| Cleanup legacy échoue (perm denied, fichier locked) | Best-effort dans `TrySwallow`. L'utilisateur garde un install dupliqué cosmétique mais rien de fonctionnellement cassé. Le raccourci Velopack remplace l'ancien dans Start Menu. |

## Validation manuelle avant publication

Le projet n'a pas de framework de test. Checklist à exécuter avant de pousser la première release Velopack publique :

1. **`dotnet publish` self-contained**
   - Lancer le publish localement.
   - Copier le dossier `publish/` sur une **VM Windows 11 vierge** sans .NET 8 installé.
   - Lancer `ImageViewer.exe` directement → doit fonctionner sans warning runtime missing.
   - Vérifier la taille du dossier (~80-100 Mo attendu).

2. **`vpk pack`**
   - Vérifier que `Releases/` contient `Setup.exe`, `ImageViewer-X.Y.Z-full.nupkg`, `releases.win.json`.
   - Pas de `delta.nupkg` à la première release Velopack — c'est attendu.

3. **Install neuve sur machine vierge**
   - Sur VM Windows 11 sans aucune trace d'ImageViewer : double-clic `Setup.exe`. Cliquer à travers le warning SmartScreen ("Plus d'infos" → "Exécuter quand même") — c'est attendu vu qu'on n'a pas signé.
   - Vérifier l'install à `%LOCALAPPDATA%\Dragon.ImageViewer\current\`.
   - Raccourci Start Menu présent.
   - Lancer l'app, ouvrir une image, vérifier rotation/crop/save.

4. **Cleanup legacy** *(le scénario le plus important — option C de cadrage)*
   - Sur une VM Windows 11 avec l'ancienne version installée via l'updater actuel : noter l'existence de `%LOCALAPPDATA%\Dragon Industries\` (qui contient `ImageViewer.exe` et ses dépendances) + le raccourci `Image Viewer.lnk` dans Start Menu.
   - Lancer le nouveau `Setup.exe`. Une fois l'install terminé et l'app lancée :
     - Le dossier `%LOCALAPPDATA%\Dragon Industries\` doit avoir disparu.
     - L'ancien raccourci doit avoir disparu (Velopack a recréé le sien au passage).
     - `HKCU\SOFTWARE\Dragon Industries\Image Viewer` doit être intacte (theme, taille fenêtre, langue conservés au prochain run).

5. **Auto-update bout en bout**
   - Publier `1.0.0` (première release Velopack).
   - Bumper à `1.0.1`, repacker, ré-uploader.
   - Sur une machine où la `1.0.0` est installée, forcer le check (`Settings.UpdateInterval = "day"` + reset `LastUpdateCheck`).
   - Toast d'update apparaît. Cliquer "Télécharger" → app redémarre en `1.0.1`.
   - Vérifier la taille du delta téléchargé (quelques Mo, pas 100 Mo).
   - Tester aussi le bouton **« Vérifier les mises à jour »** dans `DialogAbout` (visible sur le screenshot du DialogAbout actuel) → doit déclencher un check immédiat.

6. **Rollback automatique** *(optionnel mais rassurant)*
   - Publier une `1.0.2` qui crash intentionnellement au démarrage (`throw` dans le constructeur de `App`).
   - Auto-update dessus depuis `1.0.1`. Velopack doit détecter le crash et revenir à `1.0.1` automatiquement au lancement suivant.

7. **Mode dev**
   - F5 dans VS. App se lance, aucune exception `UpdateManager`.

## Hors scope

Ces points sont volontairement exclus de cette migration et restent ouverts pour des évolutions futures :

- **Signature de code** — assumée hors scope, à reprendre plus tard si la friction SmartScreen devient un problème (Azure Trusted Signing pressentie).
- **Channels beta / stable séparés** — `prerelease: false` figé pour l'instant. Ajouter un channel beta nécessitera un toggle utilisateur dans `DialogSettings` et `prerelease: true` selon le toggle.
- **CI/CD GitHub Actions** — workflow de release reste manuel, conformément aux instructions globales (`git local only, no push without explicit ask`).
- **Tests automatisés** — pas de framework xUnit ajouté ici. Validation purement manuelle via la checklist ci-dessus.
- **Internationalisation des messages d'erreur Velopack** — Velopack affiche ses propres dialogues système (uniquement en anglais) pendant `Setup.exe`. Hors scope.
- **Détection de canaux d'install multiples** (per-machine vs per-user) — Velopack est exclusivement per-user (`%LOCALAPPDATA%`). Pas de support `Program Files` envisagé.

## Références

- Velopack documentation : https://docs.velopack.io/
- Velopack GitHub : https://github.com/velopack/velopack
- Code à supprimer : `ImageViewer.Updater/` (dossier complet), `ImageViewer/Helpers/Update.cs`
- Code à créer : `ImageViewer/Helpers/LegacyCleanup.cs`
- Code à modifier : `ImageViewer/App.xaml.cs`, `ImageViewer/Helpers/Context.cs`, `ImageViewer/Helpers/NotificationsManger.cs`, `ImageViewer/Views/DialogAbout.xaml.cs` (recâblage du bouton « Vérifier les mises à jour »), `ImageViewer/ImageViewer.csproj`, `ImageViewer.sln`, `.gitignore`
