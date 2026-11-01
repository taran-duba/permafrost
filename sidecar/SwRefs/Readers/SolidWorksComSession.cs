using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using SwRefs.Protocol;

namespace SwRefs.Readers;

/// <summary>
/// Late-bound COM connection to SolidWorks (no interop DLL, so nothing to redistribute and no
/// SolidWorks needed to build). Attaches to a running SolidWorks if there is one; otherwise starts
/// a hidden one and closes it again on dispose. A SolidWorks that was already running is left alone.
/// </summary>
[SupportedOSPlatform("windows")]
public sealed class SolidWorksComSession : ISolidWorksSession
{
    private const string ProgId = "SldWorks.Application";

    private object? _app;
    private readonly bool _startedByUs;
    private readonly TextWriter _log;

    private SolidWorksComSession(object app, bool startedByUs, TextWriter log)
    {
        _app = app;
        _startedByUs = startedByUs;
        _log = log;
    }

    public static ISolidWorksSession Connect(TextWriter log)
    {
        var type = Type.GetTypeFromProgID(ProgId)
            ?? throw new ReferenceReaderException(ErrorCodes.Unsupported,
                "SolidWorks is not installed on this computer, so references can't be read. Install it, or add a Document Manager key in Settings.");

        if (TryGetRunning(type.GUID) is { } running)
        {
            log.WriteLine("swrefs: attached to the running SolidWorks");
            return new SolidWorksComSession(running, startedByUs: false, log);
        }

        log.WriteLine("swrefs: starting a hidden SolidWorks (this can take a minute)");
        object app;
        try
        {
            app = Activator.CreateInstance(type)!;
            ((dynamic)app).Visible = false;
        }
        catch (COMException e)
        {
            throw new ReferenceReaderException(ErrorCodes.OpenFailed,
                $"could not start SolidWorks (0x{e.HResult:X8}): {e.Message}");
        }
        return new SolidWorksComSession(app, startedByUs: true, log);
    }

    public object? GetDocumentDependencies(string path) =>
        ((dynamic)(_app ?? throw new ObjectDisposedException(nameof(SolidWorksComSession))))
            .GetDocumentDependencies2(path, false, false, false);

    public void Dispose()
    {
        var app = _app;
        _app = null;
        if (app is null)
            return;
        try
        {
            if (_startedByUs)
                ((dynamic)app).ExitApp();
        }
        catch (COMException e)
        {
            _log.WriteLine($"swrefs: closing SolidWorks failed: 0x{e.HResult:X8} {e.Message}");
        }
        finally
        {
            if (Marshal.IsComObject(app))
                Marshal.ReleaseComObject(app);
        }
    }

    private static object? TryGetRunning(Guid clsid)
    {
        var id = clsid;
        return GetActiveObject(ref id, IntPtr.Zero, out var obj) == 0 ? obj : null;
    }

    [DllImport("oleaut32.dll")]
    private static extern int GetActiveObject(ref Guid rclsid, IntPtr reserved,
        [MarshalAs(UnmanagedType.IUnknown)] out object? obj);
}
