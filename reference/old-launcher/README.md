# Reference material from GenLauncher 1.0.1.1

Kept from the old WPF launcher (https://github.com/soheilous/GenLauncher, commit 22d1bea) for features
the new launcher has not ported yet. Nothing here is built or shipped.

| File | Needed for |
|---|---|
| `translations/Strings.*.resx` | Localisation (en, ru, de, es, fr, hr, pt, tr, uk, zh, ar). Many UI strings are reusable. |
| `code/GameOptionsHandler.cs`, `options.ini` | The in-game graphics options editor (`Options.ini` in Documents). |
| `code/GentoolHandler.cs`, `d3d8.cfg` | GenTool version check / update and its windowed-mode setting. |
| `code/VulkanDllsHandler.cs` | Optional Vulkan (DXVK-style) wrapper handling. |
| `code/TimeUtility.cs` | System-clock sync check (only needed for signed S3 requests). |
| `fd.ico` | The original application icon. |

The full behaviour of the old launcher is described in `../../GENLAUNCHER-SPEC.md`.
