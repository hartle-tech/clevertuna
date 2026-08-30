using Clevertuna;

// A plain runner rather than a test framework: no packages to restore, and it
// runs the same way on a Mac and on Windows.
var failures = 0;

void Check(string name, Func<string?> body)
{
    string? why;
    try { why = body(); }
    catch (Exception e) { why = e.Message; }
    if (why is null) { Console.WriteLine($"ok    {name}"); }
    else { Console.WriteLine($"FAIL  {name}\n      {why}"); failures++; }
}

var layoutPath = Path.Combine(AppContext.BaseDirectory, "clvx-s-layout.json");
var layout = KeyLayout.Load(layoutPath);

Check("the layout table loads", () =>
    layout.Rows.Count == 6 ? null : $"expected six rows, found {layout.Rows.Count}");

Check("the deck has every key", () =>
    layout.KeyCount == 84 ? null : $"expected 84 keys, found {layout.KeyCount}");

Check("no key runs off the deck", () =>
{
    foreach (var row in layout.Rows)
    {
        if (row.Y + row.H > layout.Unit.Height + 0.01) return "a row runs past the bottom";
        foreach (var k in row.Keys)
            if (k.X + k.W > layout.Unit.Width + 0.01) return $"{k.Label} runs past the right edge";
    }
    return null;
});

Check("no two keys in a row overlap", () =>
{
    foreach (var row in layout.Rows)
    {
        var spans = row.Keys.Select(k => (lo: k.X, hi: k.X + k.W)).OrderBy(s => s.lo).ToList();
        for (var i = 1; i < spans.Count; i++)
            if (spans[i - 1].hi > spans[i].lo + 0.001) return "two keys sit on top of each other";
    }
    return null;
});

// The whole point of this keyboard, and the thing every drawing gets wrong.
Check("the touch surface lies over the keys", () =>
{
    var pad = layout.ZoneNamed("touchpad");
    if (pad is null) return "there is no touch surface";
    if (pad.Y is not double y || pad.H is not double h) return "the touch surface has no extent";
    var fnRowBottom = layout.Rows[0].Y + layout.Rows[0].H;
    if (y < fnRowBottom - 0.06) return "the touch surface starts above the letter rows";
    if (y + h > layout.Unit.Height + 0.01) return "the touch surface runs off the deck";
    return null;
});

Check("the sliders lie along the function row", () =>
{
    var fnRowBottom = layout.Rows[0].Y + layout.Rows[0].H;
    foreach (var id in new[] { "leftSlider", "rightSlider" })
    {
        var s = layout.ZoneNamed(id);
        if (s is null) return $"{id} is missing";
        if (s.Y is not double y || s.H is not double h) return $"{id} has no extent";
        if (y + h > fnRowBottom + 0.01) return $"{id} hangs below the function row";
    }
    return null;
});

Check("F and J carry homing bars", () =>
{
    var homing = layout.Rows.SelectMany(r => r.Keys).Where(k => k.IsHoming).Select(k => k.Label).ToList();
    return homing.Count == 2 && homing.Contains("F") && homing.Contains("J")
        ? null : $"expected F and J, found {string.Join(", ", homing)}";
});

Console.WriteLine(failures == 0 ? "\nall checks passed" : $"\n{failures} failed");
return failures == 0 ? 0 : 1;
