namespace SwRefs.Protocol;

/// <summary>Values of the "code" field in error responses.</summary>
public static class ErrorCodes
{
    /// <summary>The requested file does not exist.</summary>
    public const string NotFound = "NOT_FOUND";
    /// <summary>Not a .sldasm/.sldprt/.slddrw, or SolidWorks is not available.</summary>
    public const string Unsupported = "UNSUPPORTED";
    /// <summary>The file exists but could not be read.</summary>
    public const string OpenFailed = "OPEN_FAILED";
    /// <summary>Malformed JSON, unknown op, or missing/invalid fields.</summary>
    public const string BadRequest = "BAD_REQUEST";

    public static readonly IReadOnlySet<string> All = new HashSet<string> { NotFound, Unsupported, OpenFailed, BadRequest };
}
