using System.Text;
using SwRefs.Protocol;
using SwRefs.Readers;

namespace SwRefs;

public static class Program
{
    /// <summary>Path to a fake-reader fixture; when set, SolidWorks is not used.</summary>
    public const string FakeEnvVar = "CADSYNC_SWREFS_FAKE";

    // SolidWorks is a COM server; keep its calls on an STA thread.
    [STAThread]
    public static int Main()
    {
        var utf8 = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);
        var stdout = new StreamWriter(Console.OpenStandardOutput(), utf8) { AutoFlush = false };
        var stdin = new StreamReader(Console.OpenStandardInput(), utf8);
        var log = Console.Error;
        // Stdout carries responses only: anything else that writes to Console.Out goes to stderr.
        Console.SetOut(log);

        using var reader = CreateReader(Environment.GetEnvironmentVariable, log);
        log.WriteLine($"swrefs {ProtocolInfo.Version} ready (reader: {reader.Name})");
        Server.Run(stdin, stdout, new RequestHandler(reader, log));
        return 0;
    }

    public static IReferenceReader CreateReader(Func<string, string?> env, TextWriter log)
    {
        var fixture = env(FakeEnvVar);
        if (!string.IsNullOrWhiteSpace(fixture))
        {
            try
            {
                return FakeReferenceReader.FromFile(fixture);
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException or FormatException)
            {
                var message = $"fake fixture {fixture} could not be loaded: {e.Message}";
                log.WriteLine($"swrefs: {message}");
                return new UnavailableReferenceReader(ErrorCodes.OpenFailed, message);
            }
        }

        if (!OperatingSystem.IsWindows())
            return new UnavailableReferenceReader(ErrorCodes.Unsupported, "reading SolidWorks files only works on Windows");

        return new SolidWorksApiReferenceReader(
            () => OperatingSystem.IsWindows() ? SolidWorksComSession.Connect(log) : throw new PlatformNotSupportedException(), log);
    }
}
