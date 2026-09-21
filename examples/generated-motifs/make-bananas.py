#!/usr/bin/env python3
"""Three pop-art bananas on transparent backgrounds.

The style block is shared verbatim so the three read as one illustrator —
the same reason the earlier fruit set cohered.
"""
import base64, json, os, sys, urllib.request

KEY_PATH = os.path.expanduser("~/.config/daily-automaton/openai.key")
OUT = "/Users/leon/Projects/pattern-engine/examples/generated-motifs"

STYLE = (
    "Illustrated pop-art style {subject}. Bold uniform black outline, flat "
    "saturated colour fills with no gradients, one darker tone for shadow and "
    "one lighter tone for highlight, in the manner of a 1960s comic panel. "
    "Clean vector-like shapes, confident linework, no halftone dots, no text, "
    "no speech bubbles, no background whatsoever. A single isolated subject "
    "centred in the frame, cut out with nothing behind it."
)

SUBJECTS = {
    "20-banana-peeled": "a single banana half-peeled, the peel folded down in three strips",
    "21-banana-whole": "a single whole unpeeled banana, gently curved",
    "22-banana-bunch": "a bunch of five bananas joined at the stem",
}


def generate(name):
    key = open(KEY_PATH).read().strip()
    body = json.dumps({
        "model": "gpt-image-1",
        "prompt": STYLE.format(subject=SUBJECTS[name]),
        "size": "1024x1024",
        "quality": "medium",
        # The whole point: artwork that arrives already cut out takes the
        # ExistingAlpha path through ingest with no matting at all.
        "background": "transparent",
        "output_format": "png",
        "n": 1,
    }).encode()
    req = urllib.request.Request(
        "https://api.openai.com/v1/images/generations", data=body,
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=300) as r:
            payload = json.load(r)
    except urllib.error.HTTPError as e:
        print(f"HTTP {e.code}: {e.read().decode()[:300].replace(key, '<redacted>')}")
        return
    raw = base64.b64decode(payload["data"][0]["b64_json"])
    open(os.path.join(OUT, f"{name}.png"), "wb").write(raw)
    u = payload.get("usage", {})
    print(f"{name}.png  {len(raw)//1024} KB  ({u.get('output_tokens','?')} output tokens)")


if __name__ == "__main__":
    for n in (sys.argv[1:] or list(SUBJECTS)):
        generate(n)
