# Themes and the scheme file

- [The fifteen](#the-fifteen)
- [Rolling one](#rolling-one)
- [Save a look, or back the keyboard up?](#save-a-look-or-back-the-keyboard-up)
- [The scheme file](#the-scheme-file)

## The fifteen

Fifteen ship with the tool, five each in three groups — steady, breathing and
moving — so the keyboard has a look before you have configured anything. Five,
not fifty: a picker is for choosing, and the ones that survived are the ones
that do something the others do not rather than the same idea in another
colour.

```bash
clevertuna theme list        # names, colours and one line each
clevertuna theme reef        # shallow water, moving slowly
```

## Rolling one

```bash
clevertuna random            # roll one on the spot
clevertuna random --seed 8   # …and roll that exact one again
```

A roll is seeded, and the seed is printed and carried in the theme's name
(`Teal Wave 6f2a`), so a good one is never lost to the next click.

It rolls for movement: about 40% waves, 22% cycles, 16% breaths, 15% auroras
and only 7% steady colours — nobody presses a dice button hoping for a still
one. Palettes are built by hue family and no two members come within 45° of
each other, because three colours inside 64° read as one colour that went wrong
rather than as a scheme. Roughly a third of rolls give the touchpad or the
sliders their own treatment, in the complement of the palette's own base.

## Save a look, or back the keyboard up?

They sound alike and are not, so each says which it is:

| | What it writes | What it is for |
|---|---|---|
| **Save This Look** | the lighting only, named, into your gallery | picking it again from Yours, or sending it to someone |
| **Export a Backup** | **every** setting, verbatim — gestures, touch zones, key maps, the lot | `clevertuna import` before you experiment |

A saved theme is a small file describing four zones. A backup is the keyboard's
entire configuration, about a kilobyte of it, and restoring one puts back
things this tool does not otherwise touch.

## The scheme file

Plain JSON, one object per zone, one effect per zone:

```json
{
  "clevertuna_backlight": 1,
  "backlight": {
    "keyboard": {
      "colorWave": {
        "colorLinePicker": {
          "markersNumber": 5,
          "markersArray": [
            { "color": { "red": 255, "green": 83, "blue": 83 }, "position": 5 },
            { "color": { "red": 0, "green": 200, "blue": 255 }, "position": 29 }
          ]
        },
        "period": 3000, "direction": 270, "length": 1000
      },
      "interactiveAnimation": { "enable": true, "duration": 3 },
      "transparency": 0
    }
  }
}
```

Zones are `keyboard`, `touchpad`, `leftSlider`, `rightSlider`. Effects are
`solidColor`, `breathing`, `colorCycle`, `colorWave`, `aurora`. Up to five
markers; positions 0–100; `direction` in degrees; `period` in milliseconds.
Sliders take only `enable` on their interactive animation.

There is a worked example in [`examples/`](../examples/).

> **A gradient angle and the stored direction turn opposite ways** — the file's
> `direction` is `(180 − angle) mod 360`, a mirror rather than an offset. The
> conversion lives in `src/effects.rs` so nothing else has to know.
