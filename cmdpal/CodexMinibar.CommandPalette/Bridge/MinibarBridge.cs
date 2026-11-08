using System;
using System.Diagnostics;
using System.IO;
using System.Net.Sockets;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace CodexMinibar.CommandPalette.Bridge;

/// <summary>
/// Thrown when Minibar is not running or its loopback bridge does not answer.
/// </summary>
public sealed class BridgeUnavailableException : Exception
{
    public BridgeUnavailableException()
    {
    }

    public BridgeUnavailableException(string message)
        : base(message)
    {
    }

    public BridgeUnavailableException(string message, Exception inner)
        : base(message, inner)
    {
    }
}

/// <summary>
/// Client for the loopback bridge in `src/surfaces/streamdeck.rs`: one JSON line per
/// request, one JSON line back, authenticated by the endpoint file's token.
/// </summary>
public static class MinibarBridge
{
    private const int Protocol = 1;
    private const int MaxReplyBytes = 4 * 1024 * 1024;
    private static readonly TimeSpan Timeout = TimeSpan.FromMilliseconds(1500);

    private static string AppData => Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);

    private static string EndpointPath =>
        Path.Combine(AppData, "Codex Minibar", "Codex Minibar", "config", "streamdeck-bridge.json");

    private static string FallbackExecutable => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Codex Minibar",
        "codex-minibar.exe");

    public static Task<Reply> PaletteAsync() => SendAsync("palette", null, "palette");

    public static Task<Reply> OpenPopupAsync(string? provider) => SendAsync("open_popup", provider, "accepted");

    public static Task<Reply> RefreshAsync() => SendAsync("refresh", null, "accepted");

    public static Task<Reply> OpenSettingsAsync() => SendAsync("open_settings", null, "accepted");

    /// <summary>Starts Minibar; false when no executable could be found.</summary>
    public static bool Launch()
    {
        var executable = FallbackExecutable;
        try
        {
            var recorded = LoadEndpoint().Executable;
            if (!string.IsNullOrEmpty(recorded))
            {
                executable = recorded;
            }
        }
        catch (BridgeUnavailableException)
        {
            // A stale or missing endpoint is the usual reason to launch.
        }

        if (!File.Exists(executable))
        {
            return false;
        }

        try
        {
            using var process = Process.Start(new ProcessStartInfo(executable) { UseShellExecute = true });
            return process is not null;
        }
        catch (System.ComponentModel.Win32Exception)
        {
            return false;
        }
    }

    private static Endpoint LoadEndpoint()
    {
        Endpoint? endpoint;
        try
        {
            endpoint = JsonSerializer.Deserialize(File.ReadAllText(EndpointPath), BridgeJson.Default.Endpoint);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            throw new BridgeUnavailableException("Codex Minibar is not running", error);
        }

        if (endpoint is null || endpoint.Protocol != Protocol || endpoint.Port <= 0
            || string.IsNullOrEmpty(endpoint.Host) || string.IsNullOrEmpty(endpoint.Token))
        {
            throw new BridgeUnavailableException("invalid bridge endpoint");
        }

        return endpoint;
    }

    private static async Task<Reply> SendAsync(string op, string? provider, string expected)
    {
        var endpoint = LoadEndpoint();
        using var cancel = new CancellationTokenSource(Timeout);
        try
        {
            using var client = new TcpClient();
            await client.ConnectAsync(endpoint.Host, endpoint.Port, cancel.Token).ConfigureAwait(false);
            using var stream = client.GetStream();
            await stream.WriteAsync(Request(op, provider, endpoint.Token), cancel.Token).ConfigureAwait(false);

            var line = await ReadLineAsync(stream, cancel.Token).ConfigureAwait(false);
            var reply = JsonSerializer.Deserialize(line, BridgeJson.Default.Reply)
                ?? throw new BridgeUnavailableException("empty bridge reply");
            if (!reply.Ok || reply.Type != expected)
            {
                throw new BridgeUnavailableException(reply.Error ?? $"{op} request failed");
            }

            return reply;
        }
        catch (Exception error) when (error is SocketException or IOException or OperationCanceledException or JsonException)
        {
            throw new BridgeUnavailableException("Codex Minibar did not answer", error);
        }
    }

    private static byte[] Request(string op, string? provider, string token)
    {
        using var buffer = new MemoryStream();
        using (var writer = new Utf8JsonWriter(buffer))
        {
            writer.WriteStartObject();
            writer.WriteString("op", op);
            writer.WriteString("token", token);
            if (op == "open_popup")
            {
                if (provider is null)
                {
                    writer.WriteNull("provider");
                }
                else
                {
                    writer.WriteString("provider", provider);
                }
            }

            writer.WriteEndObject();
        }

        buffer.WriteByte((byte)'\n');
        return buffer.ToArray();
    }

    private static async Task<byte[]> ReadLineAsync(NetworkStream stream, CancellationToken cancel)
    {
        using var line = new MemoryStream();
        var chunk = new byte[16 * 1024];
        while (true)
        {
            var read = await stream.ReadAsync(chunk, cancel).ConfigureAwait(false);
            if (read == 0)
            {
                break;
            }

            var end = Array.IndexOf(chunk, (byte)'\n', 0, read);
            line.Write(chunk, 0, end < 0 ? read : end);
            if (end >= 0)
            {
                break;
            }

            if (line.Length > MaxReplyBytes)
            {
                throw new IOException("bridge reply too large");
            }
        }

        if (line.Length == 0)
        {
            throw new IOException("bridge closed without a reply");
        }

        return line.ToArray();
    }

    internal static string Describe(Exception error) =>
        error is BridgeUnavailableException ? error.Message : $"Codex Minibar failed: {error.Message}";
}
