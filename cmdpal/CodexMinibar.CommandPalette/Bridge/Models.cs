using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace CodexMinibar.CommandPalette.Bridge;

/// <summary>`streamdeck-bridge.json`, written by the running Minibar.</summary>
public sealed class Endpoint
{
    public int Protocol { get; set; }

    public string Host { get; set; } = string.Empty;

    public int Port { get; set; }

    public string Token { get; set; } = string.Empty;

    public string Executable { get; set; } = string.Empty;
}

/// <summary>
/// Any bridge reply: `palette` fills the bands and providers, `accepted` and
/// `error` only the status. Mirrors `Response` in `src/surfaces/streamdeck.rs`.
/// </summary>
public sealed class Reply
{
    public string Type { get; set; } = string.Empty;

    public bool Ok { get; set; }

    public string? Error { get; set; }

    public List<Band> Bands { get; set; } = [];

    public List<ProviderSummary> Providers { get; set; } = [];
}

/// <summary>Mirrors `cmdpal::IconPair`: PNG paths tinted for each theme.</summary>
public sealed class IconPair
{
    public string Light { get; set; } = string.Empty;

    public string Dark { get; set; } = string.Empty;
}

/// <summary>Mirrors `cmdpal::Band`.</summary>
public sealed class Band
{
    public string Id { get; set; } = string.Empty;

    public string? Provider { get; set; }

    public string Name { get; set; } = string.Empty;

    public string Title { get; set; } = string.Empty;

    public string Subtitle { get; set; } = string.Empty;

    public IconPair? Icon { get; set; }
}

/// <summary>Mirrors `cmdpal::Quota`.</summary>
public sealed class Quota
{
    public string Label { get; set; } = string.Empty;

    public string ShortLabel { get; set; } = string.Empty;

    public string Value { get; set; } = string.Empty;

    public string ValueText { get; set; } = string.Empty;

    public float? Progress { get; set; }

    public string? ResetIn { get; set; }

    public string? ResetAt { get; set; }
}

/// <summary>Mirrors `cmdpal::ProviderSummary`.</summary>
public sealed class ProviderSummary
{
    public string Id { get; set; } = string.Empty;

    public string Kind { get; set; } = string.Empty;

    public string Name { get; set; } = string.Empty;

    public List<string> Keywords { get; set; } = [];

    public string? Plan { get; set; }

    public string Headline { get; set; } = string.Empty;

    public List<Quota> Quotas { get; set; } = [];

    public string? Today { get; set; }

    public IconPair? Icon { get; set; }
}

/// <summary>The bands seen last, so pinned bands survive Minibar being closed.</summary>
public sealed class BandCache
{
    public List<Band> Bands { get; set; } = [];
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(Endpoint))]
[JsonSerializable(typeof(Reply))]
[JsonSerializable(typeof(BandCache))]
internal sealed partial class BridgeJson : JsonSerializerContext
{
}
