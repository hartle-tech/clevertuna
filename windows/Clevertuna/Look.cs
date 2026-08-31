using System.Text.Json.Serialization;

namespace Clevertuna;

/// <summary>
/// What the keyboard is showing. The shape `clevertuna look` prints, field for
/// field — written against the real output, because the macOS app was written
/// against an idea of it and spent an hour looking like a dead Bluetooth
/// connection when it was a missing key.
/// </summary>
public sealed record LookModel(
    [property: JsonPropertyName("ranges")] LookModel.RangeSet Ranges,
    [property: JsonPropertyName("zones")] Dictionary<string, LookModel.Zone> Zones,
    [property: JsonPropertyName("typing")] LookModel.Reactive Typing,
    [property: JsonPropertyName("gesture")] LookModel.Reactive Gesture)
{
    public sealed record RangeSet(
        [property: JsonPropertyName("speed")] int[] Speed,
        [property: JsonPropertyName("length")] int[] Length,
        [property: JsonPropertyName("angle")] int[] Angle,
        [property: JsonPropertyName("brightness")] int[] Brightness,
        [property: JsonPropertyName("opacity")] int[] Opacity,
        [property: JsonPropertyName("markers")] int Markers);

    public sealed record Stop(
        [property: JsonPropertyName("color")] string Color,
        [property: JsonPropertyName("position")] int Position,
        [property: JsonPropertyName("opacity")] int Opacity);

    /// <summary>
    /// What an effect can be given. A solid colour has no speed to set and no
    /// gradient to spread; offering those invites a change the keyboard ignores.
    /// </summary>
    public sealed record Offer(
        [property: JsonPropertyName("key")] string Key,
        [property: JsonPropertyName("label")] string Label,
        [property: JsonPropertyName("animated")] bool Animated,
        [property: JsonPropertyName("colours")] bool Colours,
        [property: JsonPropertyName("gradient")] bool Gradient,
        [property: JsonPropertyName("length")] bool Length,
        [property: JsonPropertyName("speed")] bool Speed);

    public sealed record Zone(
        [property: JsonPropertyName("effect")] string Effect,
        [property: JsonPropertyName("stops")] IReadOnlyList<Stop> Stops,
        [property: JsonPropertyName("swatch")] IReadOnlyList<string> Swatch,
        [property: JsonPropertyName("preview")] IReadOnlyList<string> Preview,
        [property: JsonPropertyName("brightness")] int Brightness,
        [property: JsonPropertyName("opacity")] int Opacity,
        [property: JsonPropertyName("speed")] int Speed,
        [property: JsonPropertyName("length")] int Length,
        [property: JsonPropertyName("angle")] int Angle,
        [property: JsonPropertyName("anglesFree")] bool AnglesFree,
        [property: JsonPropertyName("offers")] IReadOnlyList<Offer> Offers)
    {
        public Offer? CurrentOffer => Offers.FirstOrDefault(o => o.Key == Effect);
    }

    public sealed record Reactive(
        [property: JsonPropertyName("enabled")] bool Enabled,
        [property: JsonPropertyName("color")] string Color,
        [property: JsonPropertyName("amount")] int Amount,
        [property: JsonPropertyName("label")] string Label,
        [property: JsonPropertyName("min")] int Min,
        [property: JsonPropertyName("max")] int Max);
}

public static class Zones
{
    /// The order they read in on the hardware: the keys, the surface over them,
    /// then the strips.
    public static readonly string[] Order = ["keyboard", "touchpad", "leftSlider", "rightSlider"];

    public static string Short(string id) => id switch
    {
        "keyboard" => "Keys",
        "touchpad" => "Pad",
        "leftSlider" => "Left",
        "rightSlider" => "Right",
        _ => id,
    };

    public static string Name(string id) => id switch
    {
        "keyboard" => "Keys",
        "touchpad" => "Touchpad",
        "leftSlider" => "Left slider",
        "rightSlider" => "Right slider",
        _ => id,
    };
}

public static class Effects
{
    public static string Label(string key) => key switch
    {
        "colorWave" => "Colour wave",
        "colorCycle" => "Colour cycle",
        "breathing" => "Breathing",
        "aurora" => "Aurora",
        "solidColor" => "Solid colour",
        _ => key,
    };
}
