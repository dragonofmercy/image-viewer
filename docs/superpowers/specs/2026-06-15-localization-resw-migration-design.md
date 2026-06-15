# Design - Migrate localization to standard WinUI resources (.resw) via a Culture facade

Date: 2026-06-15
Status: Approved

## Goal

Replace the custom reflection-based localization (`Strings/*.cs` dictionaries) with the
standard WinUI / MRT Core resource system (`.resw` + `resources.pri`), while keeping the
`Culture.GetString("KEY")` access point so the ~61 existing call sites are untouched.
Then ship Simplified Chinese, German, Spanish and Italian in addition to English and French.

## Decisions (locked)

- Facade approach: keep `Culture.GetString(key)` and `Culture.GetAvailableLanguages()` as the
  public API; reimplement their internals on MRT Core. No `x:Uid` conversion (deferred).
- Languages: `en-US` (default), `fr-FR`, `zh-Hans` (Simplified), `de-DE`, `es-ES`, `it-IT`.
- `Settings.Language` stores a BCP-47 tag (e.g. `fr-FR`, `zh-Hans`) or `""` (follow system).
- No Traditional Chinese, no ICU/plurals, no x:Uid.

## Existing facts this relies on (verified)

- 61 `Culture.GetString` call sites across XAML and code; keys contain no `.` so each maps
  1:1 to a `.resw` resource read by name.
- `Culture.Init()` is called once in the `App` constructor.
- `Settings.Language` is a registry string (default `""`).
- `DialogSettings` builds the picker from `Culture.GetAvailableLanguages()` and
  `new CultureInfo(iso).NativeName.UcFirst()`, lower-cases the stored value (line 36), and
  writes `Settings.Language` on change. The first picker entry is the system default (`""`).
- `Extensions.ToUpdateDate` uses `Culture.GetString("ABOUT_LABEL_LAST_UPDATE_NEVER")` - keeps
  working through the facade.

## Components and changes

### Resource files

`Strings/en-US/Resources.resw`, `fr-FR/`, `zh-Hans/`, `de-DE/`, `es-ES/`, `it-IT/` - each
holding the full key set (~56 keys, same names as today). `en-US` is the default. The csproj
declares `<DefaultLanguage>en-US</DefaultLanguage>` and includes the `.resw` as `PRIResource`
(verify the WinAppSDK auto-glob picks them up; add an explicit `PRIResource` item if not).

### `Culture` (reimplemented facade, Helpers/Culture.cs)

Backed by MRT Core (the reliable path for unpackaged apps):

- `Init()`: create `Microsoft.Windows.ApplicationModel.Resources.ResourceManager`, create a
  `ResourceContext`; if `Settings.Language` is non-empty set
  `context.QualifierValues["Language"] = Settings.Language`. Cache manager + context.
- `GetString(key)`: `MainResourceMap.TryGetValue($"Resources/{key}", context)`; return the
  value, or `[key]` when missing (preserves today's fallback and the `[KEY]` test).
- `GetAvailableLanguages()`: return the static supported-tag list
  (`fr-FR`, `en-US`, `zh-Hans`, `de-DE`, `es-ES`, `it-IT`).
- The reflection over `ImageViewer.Strings` is removed.

### Settings / picker

- `DialogSettings` line 36: store the exact tag (drop `.ToLower()`), so `zh-Hans` survives.
- Old short stored values (`fr`, `en`) still resolve via MRT language fallback, so upgrades do
  not break.

### Removals

Delete `Strings/en.cs` and `Strings/fr.cs` (replaced by `.resw`). No other dependency changes.

### Tests

Rewrite `CultureTests` to parse the committed `.resw` XML files and assert every language's
`<data name>` set equals `en-US`'s. The test locates `ImageViewer/Strings` by walking up from
the test base directory. Keep a `[KEY]`-fallback unit on `Culture.GetString` only if it can run
without MRT in the test host; otherwise drop it (the parity test is the core guarantee). MRT
runtime correctness (actual language switching) is verified manually - it cannot be reliably
exercised in the xUnit host.

## Risk and sequencing (unpackaged MRT)

The main risk is MRT Core behaving in an unpackaged app. The work is sequenced as a vertical
slice first:

1. Slice: `.resw` for `en-US` + `fr-FR` only, the `Culture` facade, picker update, remove old
   `.cs`, rewrite tests. Build green. **Manual runtime check: app starts; switching to French
   (then restart) localizes the UI.**
2. Only after the slice works: add `zh-Hans`, `de-DE`, `es-ES`, `it-IT` `.resw` + supported list
   + `.mui` keep-list.

The user runs the manual check. If MRT does not localize in unpackaged mode, revisit the
`ResourceManager`/`ResourceContext` usage before proceeding.

## Out of scope

- `x:Uid` adoption, Traditional Chinese, plurals/ICU, runtime (no-restart) language switching.

## Acceptance

- App builds green Debug and Release (`TreatWarningsAsErrors` on).
- `.resw` key-parity test passes across all six languages.
- Manual: each language appears in Settings with its native name and localizes the UI after
  restart.
