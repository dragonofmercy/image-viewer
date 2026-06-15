# Localization .resw migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move ImageViewer localization to the standard WinUI MRT/.resw system behind the existing `Culture.GetString` facade, then add Simplified Chinese, German, Spanish and Italian.

**Architecture:** Each language is a `Strings/<tag>/Resources.resw`. `Culture` is reimplemented on MRT Core (`Microsoft.Windows.ApplicationModel.Resources.ResourceManager` + `ResourceContext`), keeping `GetString(key)` / `GetAvailableLanguages()` so the ~61 call sites are untouched. Language override comes from `Settings.Language` (BCP-47 tag) via `ResourceContext.QualifierValues["Language"]`.

**Tech Stack:** WinUI 3 / WindowsAppSDK 1.8 MRT Core, .NET 10, C# 12.

---

## File structure

- Create: `ImageViewer/Strings/en-US/Resources.resw`, `fr-FR/`, `zh-Hans/`, `de-DE/`, `es-ES/`, `it-IT/Resources.resw`.
- Modify: `ImageViewer/ImageViewer.csproj` - `<DefaultLanguage>`, `.mui` keep-lists.
- Rewrite: `ImageViewer/Helpers/Culture.cs` - MRT facade.
- Modify: `ImageViewer/Views/DialogSettings.xaml.cs` - keep exact language tag.
- Delete: `ImageViewer/Strings/en.cs`, `ImageViewer/Strings/fr.cs`.
- Rewrite: `ImageViewer.Tests/CultureTests.cs` - parse `.resw` for key parity.

**Conventions:** ASCII comments/identifiers; translated VALUES use language letters (umlauts, accents, CJK) but no curly quotes / em-dash / en-dash; ASCII `...` for ellipses; keep `{0}` verbatim. Files with non-ASCII MUST be UTF-8 (with BOM for `.cs`; `.resw` declares `encoding="utf-8"`). Commit with the repo-local identity (already set); end every commit body with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. No push, no --amend/--no-verify. `TreatWarningsAsErrors` is ON.

**Commands:**
- App build: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
- Release build: `dotnet build ImageViewer\ImageViewer.csproj -c Release -p:Platform=x64`
- Tests: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`

## .resw file template

Every `Resources.resw` uses this exact header, then one `<data>` per key, then `</root>`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<root>
  <resheader name="resmimetype">
    <value>text/microsoft-resx</value>
  </resheader>
  <resheader name="version">
    <value>2.0</value>
  </resheader>
  <resheader name="reader">
    <value>System.Resources.ResXResourceReader, System.Windows.Forms, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089</value>
  </resheader>
  <resheader name="writer">
    <value>System.Resources.ResXResourceWriter, System.Windows.Forms, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089</value>
  </resheader>
  <data name="KEY_NAME" xml:space="preserve">
    <value>The translated text</value>
  </data>
  <!-- ... one data element per key ... -->
</root>
```

XML escaping: none of the values contain `&`, `<` or `>`, so no escaping is needed. Keep apostrophes as-is. Keep `{0}` literally.

---

## Task 1: Vertical slice - en-US + fr-FR on the MRT facade

End state: the app runs fully on `.resw` for English and French; old `.cs` strings removed; parity test parses `.resw`. After this task the USER manually verifies runtime language switching before Task 2.

**Files:** create `Strings/en-US/Resources.resw`, `Strings/fr-FR/Resources.resw`; modify csproj, `Culture.cs`, `DialogSettings.xaml.cs`; delete `Strings/en.cs`, `Strings/fr.cs`; rewrite `CultureTests.cs`.

- [ ] **Step 1: Create en-US and fr-FR Resources.resw**

Create `ImageViewer/Strings/en-US/Resources.resw` using the template header, with one `<data name="K"><value>V</value></data>` per entry, where K/V are EXACTLY the key/value pairs currently in `ImageViewer/Strings/en.cs` (read that file and port all entries verbatim - same keys, same English text).

Create `ImageViewer/Strings/fr-FR/Resources.resw` the same way from `ImageViewer/Strings/fr.cs` (port all entries verbatim - same keys, same French text, keep the existing accents).

Both files must contain the IDENTICAL set of key names (they already match in en.cs/fr.cs). Save as UTF-8.

- [ ] **Step 2: Declare DefaultLanguage and ensure .resw inclusion in csproj**

In `ImageViewer/ImageViewer.csproj`, add inside the first `<PropertyGroup>` (near `<RootNamespace>`):

```xml
        <DefaultLanguage>en-US</DefaultLanguage>
```

Build once (Step 6) to confirm the WinAppSDK auto-includes `**/*.resw` as `PRIResource`. If resources do not resolve (the manual check in Step 8 shows `[KEY]` everywhere), add this ItemGroup and rebuild:

```xml
    <ItemGroup>
        <PRIResource Include="Strings\**\*.resw" />
    </ItemGroup>
```

- [ ] **Step 3: Reimplement Culture.cs as an MRT facade**

Replace the entire contents of `ImageViewer/Helpers/Culture.cs` with:

```csharp
using System.Collections.Generic;

using Microsoft.Windows.ApplicationModel.Resources;

namespace ImageViewer.Helpers;

internal class Culture
{
    private static ResourceManager _Manager;
    private static ResourceContext _Context;

    // Supported BCP-47 tags; en-US is the default language.
    private static readonly string[] AvailableLanguages = { "en-US", "fr-FR" };

    public static void Init()
    {
        _Manager = new ResourceManager();
        _Context = _Manager.CreateResourceContext();

        // Empty Settings.Language means follow the system language (no override)
        if(!string.IsNullOrEmpty(Settings.Language))
        {
            _Context.QualifierValues["Language"] = Settings.Language;
        }
    }

    public static List<string> GetAvailableLanguages()
    {
        return new List<string>(AvailableLanguages);
    }

    public static string GetString(string key)
    {
        if(_Manager == null) return $"[{key}]";

        ResourceCandidate candidate = _Manager.MainResourceMap.TryGetValue($"Resources/{key}", _Context);
        return candidate != null ? candidate.ValueAsString : $"[{key}]";
    }
}
```

- [ ] **Step 4: Keep the exact language tag in DialogSettings**

In `ImageViewer/Views/DialogSettings.xaml.cs`, change line ~36 from:

```csharp
            AvailableLanguages.Add(new CultureInfo(languagesIso).NativeName.UcFirst(), languagesIso.ToLower());
```

to (store the exact tag, do not lower-case it):

```csharp
            AvailableLanguages.Add(new CultureInfo(languagesIso).NativeName.UcFirst(), languagesIso);
```

- [ ] **Step 5: Delete the old string classes and rewrite the test**

Delete `ImageViewer/Strings/en.cs` and `ImageViewer/Strings/fr.cs`.

Replace the entire contents of `ImageViewer.Tests/CultureTests.cs` with:

```csharp
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Xml.Linq;

using Xunit;

namespace ImageViewer.Tests;

public class CultureTests
{
    private static string StringsDir()
    {
        DirectoryInfo dir = new(AppContext.BaseDirectory);
        while (dir != null)
        {
            string candidate = Path.Combine(dir.FullName, "ImageViewer", "Strings");
            if (Directory.Exists(candidate)) return candidate;
            dir = dir.Parent;
        }

        throw new DirectoryNotFoundException("Could not locate ImageViewer/Strings from " + AppContext.BaseDirectory);
    }

    private static HashSet<string> KeysOf(string languageTag)
    {
        string path = Path.Combine(StringsDir(), languageTag, "Resources.resw");
        XDocument doc = XDocument.Load(path);

        return doc.Root.Elements("data")
            .Select(e => (string)e.Attribute("name"))
            .Where(n => n != null)
            .ToHashSet();
    }

    // Every language folder under Strings that has a Resources.resw, except the en-US baseline.
    public static IEnumerable<object[]> NonDefaultLanguages()
    {
        string root = StringsDir();
        return Directory.GetDirectories(root)
            .Where(d => File.Exists(Path.Combine(d, "Resources.resw")))
            .Select(Path.GetFileName)
            .Where(name => !string.Equals(name, "en-US", StringComparison.OrdinalIgnoreCase))
            .Select(name => new object[] { name });
    }

    [Theory]
    [MemberData(nameof(NonDefaultLanguages))]
    public void Language_HasSameKeysAsEnglish(string languageTag)
    {
        HashSet<string> en = KeysOf("en-US");
        HashSet<string> other = KeysOf(languageTag);

        string[] missing = en.Except(other).OrderBy(k => k).ToArray();
        string[] extra = other.Except(en).OrderBy(k => k).ToArray();

        Assert.True(missing.Length == 0, languageTag + " is missing keys: " + string.Join(", ", missing));
        Assert.True(extra.Length == 0, languageTag + " has extra keys: " + string.Join(", ", extra));
    }
}
```

- [ ] **Step 6: Build**

Run: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
Expected: 0 warnings, 0 errors. (If it fails on the MRT types, confirm `Microsoft.Windows.ApplicationModel.Resources` resolves - it ships with the already-referenced `Microsoft.WindowsAppSDK`.)

- [ ] **Step 7: Test**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
Expected: pass. The parity Theory runs for `fr-FR` (the only non-en-US folder so far) plus the existing Image/Extensions/Dib/NaturalStringComparer tests.

- [ ] **Step 8: Commit, then STOP for manual verification**

```bash
git add ImageViewer/Strings/en-US/Resources.resw ImageViewer/Strings/fr-FR/Resources.resw ImageViewer/ImageViewer.csproj ImageViewer/Helpers/Culture.cs ImageViewer/Views/DialogSettings.xaml.cs ImageViewer.Tests/CultureTests.cs
git rm ImageViewer/Strings/en.cs ImageViewer/Strings/fr.cs
git commit -m "Migrate localization to .resw via MRT facade (en-US, fr-FR)"
```

MANUAL CHECK (the executor cannot do this; report it as REQUIRED before Task 2): run the app, confirm English UI shows correctly, open Settings, switch to French, restart, confirm the UI is French. If the UI shows `[KEY]` placeholders, the `.resw` are not being packaged - apply the `PRIResource` ItemGroup from Step 2 and rebuild.

---

## Task 2: Add Simplified Chinese, German, Spanish, Italian

Only start once the Task 1 slice is confirmed working at runtime.

**Files:** create `Strings/zh-Hans/Resources.resw`, `de-DE/`, `es-ES/`, `it-IT/Resources.resw`; modify `Culture.cs` (supported list) and `ImageViewer.csproj` (.mui).

- [ ] **Step 1: Create the four Resources.resw**

Using the template header, create one `<data name="KEY"><value>...</value></data>` per row below, for each language file. The KEY set is identical to en-US. Save UTF-8.

Per-key values (KEY -> de-DE / es-ES / it-IT / zh-Hans):

- DEFAULT_SYSTEM_LANGUAGE -> `Systemsprache` / `Idioma del sistema` / `Lingua del sistema` / `系统语言`
- SYSTEM_PASTED_CONTENT -> `Eingefügter Inhalt` / `Contenido pegado` / `Contenuto incollato` / `粘贴的内容`
- SYSTEM_LOADING_ERROR -> `Die Anwendung kann diese Datei nicht öffnen, da das Format derzeit nicht unterstützt wird oder die Datei beschädigt ist` / `Lo sentimos, la aplicación no puede abrir este archivo porque el formato no es compatible actualmente o el archivo está dañado` / `Spiacenti, l'applicazione non può aprire questo file perché il formato attualmente non è supportato o il file è danneggiato` / `抱歉，应用程序无法打开此文件，因为该格式当前不受支持或文件已损坏`
- SYSTEM_SAVING_ERROR -> `Die Anwendung kann das Bild an diesem Speicherort nicht speichern` / `Lo sentimos, la aplicación no puede guardar la imagen en esta ubicación` / `Spiacenti, l'applicazione non può salvare l'immagine in questa posizione` / `抱歉，应用程序无法将图像保存到此位置`
- SYSTEM_OK -> `OK` / `OK` / `OK` / `确定`
- SYSTEM_LOADING -> `Wird geladen` / `Cargando` / `Caricamento` / `正在加载`
- SETTINGS_FIELD_LANGUAGE -> `Sprache` / `Idioma` / `Lingua` / `语言`
- SETTINGS_FIELD_LANGUAGE_HELP -> `Sie müssen die Anwendung neu starten, um diese Einstellung anzuwenden.` / `Debe reiniciar la aplicación para aplicar este ajuste.` / `È necessario riavviare l'applicazione per applicare questa impostazione.` / `需要重新启动应用程序才能应用此设置。`
- SETTINGS_FIELD_UPDATE_INTERVAL -> `Nach Updates suchen` / `Buscar actualizaciones` / `Cerca aggiornamenti` / `检查更新`
- SETTINGS_FIELD_UPDATE_INTERVAL_DAY -> `Täglich` / `Cada día` / `Ogni giorno` / `每天`
- SETTINGS_FIELD_UPDATE_INTERVAL_WEEK -> `Wöchentlich` / `Cada semana` / `Ogni settimana` / `每周`
- SETTINGS_FIELD_UPDATE_INTERVAL_MONTH -> `Monatlich` / `Cada mes` / `Ogni mese` / `每月`
- SETTINGS_FIELD_UPDATE_INTERVAL_MANUAL -> `Manuell` / `Manualmente` / `Manualmente` / `手动`
- FILE_INFORMATION_TITLE -> `Bildinformationen` / `Información de la imagen` / `Informazioni immagine` / `图像信息`
- FILE_INFORMATION_DIMENSIONS -> `Bildabmessungen` / `Dimensiones de la imagen` / `Dimensioni immagine` / `图像尺寸`
- FILE_INFORMATION_FOLDER_PATH -> `Ordnerpfad` / `Ruta de la carpeta` / `Percorso cartella` / `文件夹路径`
- FILE_INFORMATION_FOLDER_PATH_TIP -> `Verzeichnis im Windows-Explorer anzeigen` / `Mostrar el directorio en el Explorador de Windows` / `Mostra la cartella in Esplora risorse` / `在 Windows 资源管理器中显示目录`
- FOOTER_TOOLBAR_MENU -> `Menü` / `Menú` / `Menu` / `菜单`
- FOOTER_TOOLBAR_MENU_FILE_OPEN -> `Bild öffnen` / `Abrir imagen` / `Apri immagine` / `打开图像`
- FOOTER_TOOLBAR_MENU_FILE_INFO -> `Bildinformationen` / `Información de la imagen` / `Informazioni immagine` / `图像信息`
- FOOTER_TOOLBAR_MENU_FILE_SAVE -> `Speichern unter...` / `Guardar como...` / `Salva con nome...` / `另存为...`
- FOOTER_TOOLBAR_MENU_FILE_SAVE_FORMAT -> `{0}-Bild` / `Imagen {0}` / `Immagine {0}` / `{0} 图像`
- FOOTER_TOOLBAR_MENU_FILE_DELETE -> `Bild löschen` / `Eliminar imagen` / `Elimina immagine` / `删除图像`
- FOOTER_TOOLBAR_MENU_ABOUT -> `Über` / `Acerca de` / `Informazioni` / `关于`
- FOOTER_TOOLBAR_MENU_OPTIONS -> `Einstellungen` / `Ajustes` / `Impostazioni` / `设置`
- FOOTER_TOOLBAR_MENU_QUIT -> `Beenden` / `Salir` / `Esci` / `退出`
- FOOTER_TOOLBAR_IMAGE_ADJUST -> `An Fenster anpassen` / `Ajustar a la ventana` / `Adatta alla finestra` / `缩放以适应`
- FOOTER_TOOLBAR_IMAGE_ZOOM_100 -> `Tatsächliche Größe` / `Tamaño real` / `Dimensione reale` / `实际大小`
- FOOTER_TOOLBAR_IMAGE_PREVIOUS -> `Vorheriges Bild` / `Imagen anterior` / `Immagine precedente` / `上一张图像`
- FOOTER_TOOLBAR_IMAGE_ZOOM_IN -> `Vergrößern` / `Acercar` / `Ingrandisci` / `放大`
- FOOTER_TOOLBAR_IMAGE_ZOOM_OUT -> `Verkleinern` / `Alejar` / `Riduci` / `缩小`
- FOOTER_TOOLBAR_IMAGE_NEXT -> `Nächstes Bild` / `Imagen siguiente` / `Immagine successiva` / `下一张图像`
- FOOTER_TOOLBAR_TRANSFORM_MENU -> `Bild transformieren` / `Transformar imagen` / `Trasforma immagine` / `变换图像`
- FOOTER_TOOLBAR_TRANSFORM_CROP -> `Bild zuschneiden` / `Recortar imagen` / `Ritaglia immagine` / `裁剪图像`
- FOOTER_TOOLBAR_TRANSFORM_ROTATE_LEFT -> `Nach links drehen` / `Girar a la izquierda` / `Ruota a sinistra` / `向左旋转`
- FOOTER_TOOLBAR_TRANSFORM_ROTATE_RIGHT -> `Nach rechts drehen` / `Girar a la derecha` / `Ruota a destra` / `向右旋转`
- FOOTER_TOOLBAR_TRANSFORM_FLIP_HORZ -> `Horizontal spiegeln` / `Voltear horizontalmente` / `Capovolgi orizzontalmente` / `水平翻转`
- FOOTER_TOOLBAR_TRANSFORM_FLIP_VERT -> `Vertikal spiegeln` / `Voltear verticalmente` / `Capovolgi verticalmente` / `垂直翻转`
- TRANSFORM_CROP_TITLE -> `Zuschneidewerkzeug` / `Herramienta de recorte` / `Strumento di ritaglio` / `裁剪工具`
- TRANSFORM_CROP_RATIO -> `Seitenverhältnis` / `Proporción` / `Proporzioni` / `比例`
- TRANSFORM_CROP_FREE -> `Keine Einschränkung` / `Sin restricciones` / `Nessun vincolo` / `无约束`
- TRANSFORM_CROP_SAME -> `Originalproportionen` / `Proporciones originales` / `Proporzioni originali` / `原始比例`
- TRANSFORM_CROP_RESET -> `Zurücksetzen` / `Restablecer` / `Reimposta` / `重置`
- TRANSFORM_CROP_VALIDATE -> `Übernehmen` / `Aplicar` / `Applica` / `确认`
- ABOUT_LINK_GITHUB_REPOSITORY -> `GitHub-Repository` / `Repositorio de GitHub` / `Repository GitHub` / `GitHub 仓库`
- ABOUT_LINK_LATEST_RELEASE -> `Neueste Versionen` / `Últimas versiones` / `Ultime versioni` / `最新版本`
- ABOUT_LABEL_LAST_UPDATE -> `Zuletzt geprüft: ` / `Última comprobación: ` / `Ultimo controllo: ` / `上次检查: `
- ABOUT_LABEL_LAST_UPDATE_NEVER -> `nie` / `nunca` / `mai` / `从不`
- ABOUT_BTN_CHECK_UPDATE -> `Nach Updates suchen` / `Buscar actualizaciones` / `Cerca aggiornamenti` / `检查更新`
- ABOUT_BTN_DOWNLOAD_UPDATE -> `Update herunterladen` / `Descargar actualización` / `Scarica aggiornamento` / `下载更新`
- ABOUT_BTN_DOWNLOAD_UPDATE_DOWNLOADING -> `Wird heruntergeladen...` / `Descargando...` / `Download in corso...` / `正在下载...`
- ABOUT_BTN_DOWNLOAD_UPDATE_RETRY -> `Erneut versuchen` / `Reintentar` / `Riprova` / `重试`
- ABOUT_UPDATE_CHECKING -> `Suche nach Updates...` / `Buscando actualizaciones...` / `Ricerca aggiornamenti...` / `正在检查更新...`
- ABOUT_UPDATE_INFO_UPDATE_LATEST -> `Image Viewer ist aktuell.` / `Image Viewer está actualizado.` / `Image Viewer è aggiornato.` / `Image Viewer 已是最新版本。`
- ABOUT_UPDATE_INFO_UPDATE_AVAILABLE -> `Ein Update ist verfügbar.` / `Hay una actualización disponible.` / `È disponibile un aggiornamento.` / `有可用更新。`
- ABOUT_UPDATE_INFO_ERROR_NO_INTERNET -> `Kein Internetzugang.` / `Sin acceso a internet.` / `Nessun accesso a internet.` / `无法访问互联网。`

After creating the files, ensure UTF-8 encoding is intact:

```powershell
foreach ($t in "zh-Hans","de-DE","es-ES","it-IT") {
    $f = "ImageViewer/Strings/$t/Resources.resw"
    $text = Get-Content -Raw -Encoding utf8 $f
    [System.IO.File]::WriteAllText((Resolve-Path $f), $text, (New-Object System.Text.UTF8Encoding($false)))
}
Select-String -Path ImageViewer/Strings/zh-Hans/Resources.resw -Pattern "系统语言" -Encoding utf8
Select-String -Path ImageViewer/Strings/de-DE/Resources.resw -Pattern "Eingefügter" -Encoding utf8
```

Both `Select-String` must report a match.

- [ ] **Step 2: Extend the supported-language list**

In `ImageViewer/Helpers/Culture.cs`, change:

```csharp
    private static readonly string[] AvailableLanguages = { "en-US", "fr-FR" };
```

to:

```csharp
    private static readonly string[] AvailableLanguages = { "en-US", "fr-FR", "zh-Hans", "de-DE", "es-ES", "it-IT" };
```

- [ ] **Step 3: Extend the .mui keep-lists**

In `ImageViewer/ImageViewer.csproj`, `PostBuild` target, replace:

```xml
            <RemovingFiles Include="$(OutDir)**\*.mui" Exclude="$(OutDir)en-us\*.mui;$(OutDir)fr-FR\*.mui;$(OutDir)fr\*.mui" />
```

with:

```xml
            <RemovingFiles Include="$(OutDir)**\*.mui" Exclude="$(OutDir)en-us\*.mui;$(OutDir)fr-FR\*.mui;$(OutDir)fr\*.mui;$(OutDir)de-DE\*.mui;$(OutDir)de\*.mui;$(OutDir)es-ES\*.mui;$(OutDir)es\*.mui;$(OutDir)it-IT\*.mui;$(OutDir)it\*.mui;$(OutDir)zh-Hans\*.mui;$(OutDir)zh-CN\*.mui" />
```

In the `PostPublish` target, replace:

```xml
            <PublishMuiToRemove Include="$(PublishDir)**\*.mui" Exclude="$(PublishDir)en-us\*.mui;$(PublishDir)fr-FR\*.mui;$(PublishDir)fr\*.mui" />
```

with:

```xml
            <PublishMuiToRemove Include="$(PublishDir)**\*.mui" Exclude="$(PublishDir)en-us\*.mui;$(PublishDir)fr-FR\*.mui;$(PublishDir)fr\*.mui;$(PublishDir)de-DE\*.mui;$(PublishDir)de\*.mui;$(PublishDir)es-ES\*.mui;$(PublishDir)es\*.mui;$(PublishDir)it-IT\*.mui;$(PublishDir)it\*.mui;$(PublishDir)zh-Hans\*.mui;$(PublishDir)zh-CN\*.mui" />
```

- [ ] **Step 4: Build Debug and Release**

Run: `dotnet build ImageViewer\ImageViewer.csproj -c Debug -p:Platform=x64`
Run: `dotnet build ImageViewer\ImageViewer.csproj -c Release -p:Platform=x64`
Expected both: 0 warnings, 0 errors.

- [ ] **Step 5: Test**

Run: `dotnet test ImageViewer.Tests\ImageViewer.Tests.csproj -c Debug -p:Platform=x64`
Expected: pass; the parity Theory now runs for fr-FR, zh-Hans, de-DE, es-ES, it-IT (each must have the same keys as en-US).

- [ ] **Step 6: Commit**

```bash
git add ImageViewer/Strings/zh-Hans/Resources.resw ImageViewer/Strings/de-DE/Resources.resw ImageViewer/Strings/es-ES/Resources.resw ImageViewer/Strings/it-IT/Resources.resw ImageViewer/Helpers/Culture.cs ImageViewer/ImageViewer.csproj
git commit -m "Add zh-Hans, de-DE, es-ES, it-IT resource translations"
```

---

## Final verification

- [ ] Debug build green; Release build green.
- [ ] Tests green (key parity across fr-FR/zh-Hans/de-DE/es-ES/it-IT vs en-US).
- [ ] Encoding checks matched (Task 2 step 1).
- [ ] Manual (USER): each language appears in Settings with its native name (Francais / 中文 / Deutsch / Espanol / Italiano) and localizes the UI after restart; no `[KEY]` placeholders anywhere.
```
