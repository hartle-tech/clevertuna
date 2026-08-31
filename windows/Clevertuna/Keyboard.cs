using System.Text.Json;
using System.Text.Json.Serialization;

namespace Clevertuna;

/// <summary>
/// The physical keyboard, as data.
///
/// The Clevetura CLVX S is a compact US ANSI board: a half-height function row,
/// five letter rows, a six-key column down the right edge and an inverted-T
/// arrow cluster.
///
/// The part every drawing of it gets wrong: there is no separate touchpad. The
/// touch surface is a region of the key field itself, and the two sliders are
/// strips running along the F2–F6 and F7–F11 keycaps.
///
/// The table is assets/clvx-s-layout.json, shared with the Rust core and the
/// macOS app, so the keyboard is described once.
/// </summary>
public sealed record KeyLayout(
    [property: JsonPropertyName("unit")] KeyLayout.UnitSize Unit,
    [property: JsonPropertyName("rows")] IReadOnlyList<KeyLayout.Row> Rows,
    [property: JsonPropertyName("zones")] IReadOnlyList<KeyLayout.Zone> Zones)
{
    public sealed record UnitSize(
        [property: JsonPropertyName("width")] double Width,
        [property: JsonPropertyName("height")] double Height,
        [property: JsonPropertyName("keyGap")] double KeyGap);

    public sealed record Key(
        [property: JsonPropertyName("x")] double X,
        [property: JsonPropertyName("w")] double W,
        [property: JsonPropertyName("label")] string Label,
        [property: JsonPropertyName("sub")] string? Sub,
        [property: JsonPropertyName("size")] string? Size,
        [property: JsonPropertyName("homing")] bool? Homing,
        [property: JsonPropertyName("led")] bool? Led,
        [property: JsonPropertyName("space")] bool? Space)
    {
        public bool IsSpace => Space ?? false;
        public bool IsHoming => Homing ?? false;
    }

    public sealed record Row(
        [property: JsonPropertyName("y")] double Y,
        [property: JsonPropertyName("h")] double H,
        [property: JsonPropertyName("keys")] IReadOnlyList<Key> Keys);

    public sealed record Zone(
        [property: JsonPropertyName("id")] string Id,
        [property: JsonPropertyName("name")] string Name,
        [property: JsonPropertyName("shape")] string Shape,
        [property: JsonPropertyName("x")] double? X,
        [property: JsonPropertyName("y")] double? Y,
        [property: JsonPropertyName("w")] double? W,
        [property: JsonPropertyName("h")] double? H)
    {
        public bool IsField => Shape == "field";
        public bool IsStrip => Shape == "strip";
    }

    public double Aspect => Unit.Width / Unit.Height;

    public int KeyCount => Rows.Sum(r => r.Keys.Count);

    public Zone? ZoneNamed(string id) => Zones.FirstOrDefault(z => z.Id == id);

    /// <summary>
    /// Read from the file beside the executable. A keyboard app that cannot
    /// draw the keyboard has nothing to show, so a missing table is a failure
    /// rather than a silent fallback.
    /// </summary>
    public static KeyLayout Load(string? path = null)
    {
        path ??= Path.Combine(AppContext.BaseDirectory, "clvx-s-layout.json");
        var json = File.ReadAllText(path);
        return JsonSerializer.Deserialize<KeyLayout>(json)
               ?? throw new InvalidDataException($"{path} is not a layout this app understands");
    }
}
