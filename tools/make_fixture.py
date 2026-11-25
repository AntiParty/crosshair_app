"""Build tests/fixtures/opt.sav: a synthetic settings file with the same structure as the game's
EmbarkOptionSaveGame.sav but none of anyone's preferences.

    python tools/make_fixture.py <real EmbarkOptionSaveGame.sav> [out]

It keeps the real file's header, setting names and layout (so the parser and the apply logic are tested on the
real shape) and replaces every value:
  * crosshair and sight settings  -> the game's defaults, with a few fixed sample values so edits are visible
  * true / false values           -> "false"
  * numbers                       -> "1.0"
  * crosshair shape enums          -> the first/default choice
  * other "Type::Value" enums      -> kept (game constants; they reveal only which menu choice was selected)
  * any other text (region, devices, resolution ...) -> ""
  * the opaque AdditionalData blob -> zero bytes of the same length
The real file is only read, never written.
"""
import re
import struct
import sys

SRC = sys.argv[1]
OUT = sys.argv[2] if len(sys.argv) > 2 else "tests/fixtures/opt.sav"
b = open(SRC, "rb").read()


def fstr(p):
    n = struct.unpack_from("<i", b, p)[0]
    p += 4
    if n == 0:
        return "", p
    if n < 0:
        raise SystemExit("UTF-16 string in the header: unsupported here")
    return b[p:p + n - 1].decode("latin1"), p + n


def enc(s):
    if not s:
        return struct.pack("<i", 0)
    return struct.pack("<i", len(s) + 1) + s.encode("ascii") + b"\0"


def type_name(p):
    t, p = fstr(p)
    n = struct.unpack_from("<i", b, p)[0]
    p += 4
    for _ in range(n):
        _, p = type_name(p)
    return t, p


# header
p = 4 + 12 + 10
_, p = fstr(p)          # branch
p += 4                  # custom version format
n = struct.unpack_from("<i", b, p)[0]
p += 4 + n * 20
cls, p = fstr(p)
p += 1                  # class extension byte

# walk tags until the Properties map; remember the AdditionalData tag too
map_start = map_size_pos = map_end = add_start = add_end = None
while True:
    name, p = fstr(p)
    if name == "None":
        break
    ty, p = type_name(p)
    size_pos = p
    size = struct.unpack_from("<i", b, p)[0]
    p += 4
    flags = b[p]
    p += 1
    if flags & 1:
        p += 4
    if flags & 2:
        p += 16
    if ty == "MapProperty" and name == "Properties":
        map_start, map_size_pos, map_end = p, size_pos, p + size
    if ty == "ArrayProperty" and name == "AdditionalData":
        add_start, add_end = p, p + size
    p += size

q = map_start
removed, count = struct.unpack_from("<ii", b, q)
q += 8
assert removed == 0
entries = []
for _ in range(count):
    k, q = fstr(q)
    v, q = fstr(q)
    entries.append((k, v))
assert q == map_end, (q, map_end)

SAMPLES = {
    "GameplayOption.Crosshair.Center.Radius": "4.0",
    "GameplayOption.Crosshair.Center.Color.Red": "255.0",
    "GameplayOption.Crosshair.Center.Color.Green": "90.0",
    "GameplayOption.Crosshair.Center.Color.Blue": "54.0",
    "GameplayOption.Crosshair.DispersionReticle.Length": "6.0",
    "GameplayOption.Sights.ReticleSize": "1.0",
}
sys.path.insert(0, "core/src")
src = open("core/src/schema_data.rs", encoding="utf-8").read()
defaults = {}
for m in re.finditer(r'\("([^"]+)", ([-0-9.]+), [-0-9.]+, [-0-9.]+\)', src):
    defaults[m.group(1)] = m.group(2)
for m in re.finditer(r'\("([^"]+)", (true|false)\)', src):
    defaults[m.group(1)] = m.group(2)

ENUM_DEFAULT = {
    "GameplayOption.Crosshair.Center.Shape": "ECrosshairCenterShape::Dot",
    "GameplayOption.Crosshair.MeleeCenter.Shape": "ECrosshairCenterShape::Dot",
    "GameplayOption.Crosshair.StaticReticle.Shape": "ECrosshairReticleShape::Plus",
    "GameplayOption.Crosshair.DispersionReticle.Shape": "ECrosshairReticleShape::Plus",
    "GameplayOption.Crosshair.MeleeLook": "EMeleeCrosshairLook::Same",
    "GameplayOption.Sights.ReticleShape": "EReticleShape::Dot",
}
CROSSHAIR = ("GameplayOption.Crosshair.", "GameplayOption.Sights.", "GameplayOption.Accessibility.Crosshair.")
num = re.compile(r"^-?\d+(\.\d+)?$")


def sanitize(k, v):
    if k in SAMPLES:
        return SAMPLES[k]
    if k.startswith(CROSSHAIR):
        if "::" in v:
            return ENUM_DEFAULT.get(k, v)
        return defaults.get(k, "1.0" if num.match(v) else ("false" if v in ("true", "false") else ""))
    if v in ("true", "false"):
        return "false"
    if num.match(v):
        return "1.0"
    if "::" in v:
        return v
    return ""


body = struct.pack("<ii", 0, count)
for k, v in entries:
    body += enc(k) + enc(sanitize(k, v))

out = bytearray(b[:map_start]) + body + b[map_end:]
# patch the map size field; its position is unchanged because everything before the body is
out[map_size_pos:map_size_pos + 4] = struct.pack("<i", len(body))
# the AdditionalData blob sits after the map: its offset moves by the size difference
delta = len(body) - (map_end - map_start)
if add_start is not None:
    a0, a1 = add_start + delta, add_end + delta
    # array property data: i32 element count, then the bytes; keep the count, zero the bytes
    out[a0 + 4:a1] = bytes(a1 - a0 - 4)
open(OUT, "wb").write(bytes(out))
print(f"{count} settings written to {OUT} ({len(out)} bytes); AdditionalData zeroed: {add_start is not None}")
