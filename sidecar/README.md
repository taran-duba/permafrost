# SwRefs sidecar

A small .NET 8 console app that Permafrost's Rust core starts as a child process to find out which
files a SolidWorks part, assembly or drawing references. It asks SolidWorks itself through its
normal automation API (`ISldWorks.GetDocumentDependencies2`), so it needs no license key and no
interop DLL, but SolidWorks must be installed on the PC.

- `SwRefs/` – the app
- `SwRefs.Tests/` – xUnit tests (protocol, fake reader, end-to-end process run)
- `fixtures/sample-refs.json` – sample fixture for the fake reader

## Protocol

One JSON object per line on stdin; exactly one JSON line per request on stdout, in order.
Logs go to stderr only. The process exits when stdin closes. Blank lines are ignored.

```
→ {"id":"1","op":"ping"}
← {"id":"1","ok":true,"version":"0.1.0"}

→ {"id":"2","op":"refs","path":"C:\\Vault\\Robot.SLDASM"}
← {"id":"2","ok":true,"refs":[{"path":"C:\\Vault\\Arm.SLDASM","fileName":"Arm.SLDASM","exists":true,"isVirtual":false}]}

← {"id":"3","ok":false,"error":"file not found: ...","code":"NOT_FOUND"}
```

`refs` returns direct references only, deduplicated by path (case-insensitively). `path` is the
path stored in the file, and `exists` says whether a file is at that path on this machine.
Virtual components are included with `isVirtual: true`; they live inside the assembly, so ignore
`exists` for them.

| code | when |
| --- | --- |
| `BAD_REQUEST` | invalid JSON, not an object, missing/non-string `id` (response `id` is `null`), missing `op`, unknown `op`, missing or relative `path` |
| `UNSUPPORTED` | not `.sldasm`/`.sldprt`/`.slddrw`, not running on Windows, or SolidWorks isn't installed |
| `NOT_FOUND` | the file does not exist |
| `OPEN_FAILED` | the file exists but could not be read (damaged, newer SolidWorks version, SolidWorks busy or closed, …) |

## Environment variables

| variable | purpose |
| --- | --- |
| `CADSYNC_SWREFS_FAKE` | Path to a JSON fixture. When set, the fake reader answers instead of SolidWorks. |

## How it talks to SolidWorks

It connects to a running SolidWorks; if there isn't one it starts a hidden SolidWorks (the first
`refs` can take a minute) and closes it when SwRefs exits. A SolidWorks you already had open is
never closed. `ping` never contacts SolidWorks. If SolidWorks is busy, calls are retried for a few
seconds; if it closes mid-session, the next request reconnects.

If Permafrost is force-killed while it started SolidWorks, that hidden `sldworks.exe` stays
running (the next check attaches to it); end it in Task Manager if you want it gone.

The calls are late-bound COM, so the app builds on any machine with the .NET 8 SDK. Off Windows
it answers `UNSUPPORTED`. For the Tauri installer, `scripts/publish-swrefs.ps1` (`pnpm sidecar`)
publishes a self-contained single-file `win-x64` exe and copies it to
`src-tauri/binaries/SwRefs-x86_64-pc-windows-msvc.exe`; see "Build the Windows installer" in the
top-level README.

## Building and testing anywhere else (fake mode)

```bash
dotnet test sidecar/SwRefs.Tests
```

Off Windows the projects target `net8.0` (on Windows, `net8.0-windows`) and roll forward to a
newer runtime if .NET 8 is not installed. To run the sidecar against the sample fixture:

```bash
CADSYNC_SWREFS_FAKE=sidecar/fixtures/sample-refs.json dotnet run --project sidecar/SwRefs
```

Fixture format (keys match case-insensitively; a ref without `exists` is checked on disk):

```json
{
  "C:\\Vault\\Robot.SLDASM": [
    { "path": "C:\\Vault\\Arm.SLDASM", "exists": true },
    "C:\\Vault\\Base.SLDPRT",
    { "path": "Spacer^Robot.sldprt", "isVirtual": true }
  ],
  "C:\\Vault\\Corrupt.SLDASM": { "error": "OPEN_FAILED", "message": "file is damaged" }
}
```

Paths not in the fixture return `NOT_FOUND`. If the fixture cannot be loaded, the sidecar still
answers `ping`, and every `refs` returns `OPEN_FAILED` with the reason.
