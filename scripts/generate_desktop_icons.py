#!/usr/bin/env python3

import math
import shutil
import struct
import subprocess
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
ICONS_DIR = ROOT / "apps" / "desktop" / "src-tauri" / "icons"
MASTER_ICON = ICONS_DIR / "icon.png"
SIZE = 1024
AA = 1.5 / SIZE


def clamp01(value: float) -> float:
    return 0.0 if value < 0.0 else 1.0 if value > 1.0 else value


def smoothstep(edge0: float, edge1: float, value: float) -> float:
    t = clamp01((value - edge0) / (edge1 - edge0))
    return t * t * (3.0 - 2.0 * t)


def mix(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def mix_color(a, b, t: float):
    return (
        mix(a[0], b[0], t),
        mix(a[1], b[1], t),
        mix(a[2], b[2], t),
    )


def rgba(color, alpha: float):
    return (color[0], color[1], color[2], alpha)


def composite(dst, src):
    sr, sg, sb, sa = src
    dr, dg, db, da = dst
    out_a = sa + da * (1.0 - sa)
    if out_a <= 0.0:
        return (0.0, 0.0, 0.0, 0.0)
    out_r = (sr * sa + dr * da * (1.0 - sa)) / out_a
    out_g = (sg * sa + dg * da * (1.0 - sa)) / out_a
    out_b = (sb * sa + db * da * (1.0 - sa)) / out_a
    return (out_r, out_g, out_b, out_a)


def rounded_rect_distance(
    x: float, y: float, cx: float, cy: float, hx: float, hy: float, radius: float
) -> float:
    qx = abs(x - cx) - (hx - radius)
    qy = abs(y - cy) - (hy - radius)
    ox = max(qx, 0.0)
    oy = max(qy, 0.0)
    return math.hypot(ox, oy) + min(max(qx, qy), 0.0) - radius


def circle_coverage(
    x: float, y: float, cx: float, cy: float, radius: float, softness: float = AA
) -> float:
    dist = math.hypot(x - cx, y - cy) - radius
    return 1.0 - smoothstep(-softness, softness, dist)


def rounded_rect_coverage(
    x: float,
    y: float,
    cx: float,
    cy: float,
    hx: float,
    hy: float,
    radius: float,
    softness: float = AA,
) -> float:
    dist = rounded_rect_distance(x, y, cx, cy, hx, hy, radius)
    return 1.0 - smoothstep(-softness, softness, dist)


def dist_to_segment(px: float, py: float, ax: float, ay: float, bx: float, by: float) -> float:
    abx = bx - ax
    aby = by - ay
    ab_len_sq = abx * abx + aby * aby
    if ab_len_sq == 0.0:
        return math.hypot(px - ax, py - ay)
    t = ((px - ax) * abx + (py - ay) * aby) / ab_len_sq
    t = clamp01(t)
    qx = ax + abx * t
    qy = ay + aby * t
    return math.hypot(px - qx, py - qy)


def point_in_triangle(px: float, py: float, a, b, c) -> bool:
    def sign(p1, p2, p3) -> float:
        return (p1[0] - p3[0]) * (p2[1] - p3[1]) - (p2[0] - p3[0]) * (
            p1[1] - p3[1]
        )

    d1 = sign((px, py), a, b)
    d2 = sign((px, py), b, c)
    d3 = sign((px, py), c, a)
    has_neg = d1 < 0.0 or d2 < 0.0 or d3 < 0.0
    has_pos = d1 > 0.0 or d2 > 0.0 or d3 > 0.0
    return not (has_neg and has_pos)


def triangle_coverage(x: float, y: float, a, b, c, softness: float = AA) -> float:
    min_dist = min(
        dist_to_segment(x, y, a[0], a[1], b[0], b[1]),
        dist_to_segment(x, y, b[0], b[1], c[0], c[1]),
        dist_to_segment(x, y, c[0], c[1], a[0], a[1]),
    )
    signed = -min_dist if point_in_triangle(x, y, a, b, c) else min_dist
    return 1.0 - smoothstep(-softness, softness, signed)


def icon_pixel(x: float, y: float):
    pixel = (0.0, 0.0, 0.0, 0.0)

    mask = rounded_rect_coverage(x, y, 0.0, 0.0, 0.86, 0.86, 0.23)
    if mask <= 0.0:
        return pixel

    top = (0.05, 0.14, 0.24)
    bottom = (0.05, 0.34, 0.44)
    background = mix_color(top, bottom, clamp01((y + 1.0) * 0.5))

    glow_a = math.exp(-(((x + 0.45) ** 2) / 0.12 + ((y + 0.55) ** 2) / 0.08)) * 0.55
    glow_b = math.exp(-(((x - 0.45) ** 2) / 0.18 + ((y - 0.45) ** 2) / 0.10)) * 0.28
    background = mix_color(background, (0.15, 0.82, 0.92), glow_a)
    background = mix_color(background, (0.97, 0.58, 0.18), glow_b)

    pixel = composite(pixel, rgba(background, mask))

    border_outer = rounded_rect_coverage(x, y, 0.0, 0.0, 0.86, 0.86, 0.23)
    border_inner = rounded_rect_coverage(x, y, 0.0, 0.0, 0.81, 0.81, 0.20)
    border_alpha = max(border_outer - border_inner, 0.0) * 0.42
    pixel = composite(pixel, rgba((0.82, 0.96, 1.0), border_alpha))

    shadow_left = rounded_rect_coverage(
        x, y, -0.18, -0.14, 0.34, 0.24, 0.10, softness=0.035
    )
    shadow_right = rounded_rect_coverage(
        x, y, 0.21, 0.18, 0.34, 0.24, 0.10, softness=0.035
    )
    pixel = composite(pixel, rgba((0.0, 0.02, 0.06), shadow_left * 0.18))
    pixel = composite(pixel, rgba((0.0, 0.02, 0.06), shadow_right * 0.18))

    panel_left = rounded_rect_coverage(x, y, -0.21, -0.18, 0.33, 0.23, 0.10)
    panel_right = rounded_rect_coverage(x, y, 0.21, 0.17, 0.33, 0.23, 0.10)

    left_top = (0.98, 0.63, 0.24)
    left_bottom = (0.91, 0.31, 0.22)
    left_color = mix_color(left_top, left_bottom, clamp01((y + 0.5) * 0.9))

    right_top = (0.22, 0.88, 0.82)
    right_bottom = (0.09, 0.58, 0.90)
    right_color = mix_color(right_top, right_bottom, clamp01((y + 0.5) * 0.9))

    pixel = composite(pixel, rgba(left_color, panel_left))
    pixel = composite(pixel, rgba(right_color, panel_right))

    left_highlight = (
        circle_coverage(x, y, -0.34, -0.34, 0.18, softness=0.10) * panel_left * 0.20
    )
    right_highlight = (
        circle_coverage(x, y, 0.08, -0.01, 0.20, softness=0.10) * panel_right * 0.18
    )
    pixel = composite(pixel, rgba((1.0, 0.94, 0.84), left_highlight))
    pixel = composite(pixel, rgba((0.90, 1.0, 1.0), right_highlight))

    left_triangle = (
        triangle_coverage(
            x,
            y,
            (-0.32, -0.28),
            (-0.32, -0.08),
            (-0.13, -0.18),
        )
        * panel_left
    )
    right_triangle = (
        triangle_coverage(
            x,
            y,
            (0.32, 0.27),
            (0.32, 0.07),
            (0.13, 0.17),
        )
        * panel_right
    )

    pixel = composite(pixel, rgba((0.99, 1.0, 1.0), left_triangle * 0.95))
    pixel = composite(pixel, rgba((0.99, 1.0, 1.0), right_triangle * 0.95))

    beam = rounded_rect_coverage(x, y, 0.0, 0.0, 0.30, 0.07, 0.05)
    beam_gradient = mix_color(
        (0.76, 0.97, 1.0), (1.0, 0.90, 0.78), clamp01((x + 1.0) * 0.5)
    )
    beam_inner = rounded_rect_coverage(x, y, 0.0, 0.0, 0.27, 0.04, 0.03)
    beam_alpha = max(beam - beam_inner * 0.45, 0.0) * 0.55
    pixel = composite(pixel, rgba(beam_gradient, beam_alpha))

    inner_left = rounded_rect_coverage(x, y, -0.21, -0.18, 0.30, 0.20, 0.08)
    inner_right = rounded_rect_coverage(x, y, 0.21, 0.17, 0.30, 0.20, 0.08)
    accent = max(panel_left - inner_left, 0.0) * 0.18 + max(
        panel_right - inner_right, 0.0
    ) * 0.18
    pixel = composite(pixel, rgba((1.0, 1.0, 1.0), accent))

    status_left = circle_coverage(x, y, -0.42, -0.31, 0.028) * panel_left
    status_right = circle_coverage(x, y, 0.00, 0.04, 0.028) * panel_right
    pixel = composite(pixel, rgba((1.0, 0.96, 0.88), status_left * 0.85))
    pixel = composite(pixel, rgba((0.93, 1.0, 1.0), status_right * 0.85))

    return pixel


def write_png(path: Path, width: int, height: int, rows) -> None:
    raw = bytearray()
    stride = width * 4
    for y in range(height):
        raw.append(0)
        start = y * stride
        raw.extend(rows[start : start + stride])

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png = bytearray(b"\x89PNG\r\n\x1a\n")
    png.extend(chunk(b"IHDR", ihdr))
    png.extend(chunk(b"IDAT", zlib.compress(bytes(raw), 9)))
    png.extend(chunk(b"IEND", b""))
    path.write_bytes(png)


def generate_master_icon() -> None:
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    data = bytearray(SIZE * SIZE * 4)
    index = 0
    for py in range(SIZE):
        y = ((py + 0.5) / SIZE) * 2.0 - 1.0
        for px in range(SIZE):
            x = ((px + 0.5) / SIZE) * 2.0 - 1.0
            r, g, b, a = icon_pixel(x, y)
            data[index] = round(clamp01(r) * 255.0)
            data[index + 1] = round(clamp01(g) * 255.0)
            data[index + 2] = round(clamp01(b) * 255.0)
            data[index + 3] = round(clamp01(a) * 255.0)
            index += 4

    write_png(MASTER_ICON, SIZE, SIZE, data)


def generate_icon_variants() -> None:
    subprocess.run(
        [
            "cargo",
            "tauri",
            "icon",
            str(MASTER_ICON),
            "--output",
            str(ICONS_DIR),
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def verify_tool(name: str) -> None:
    if shutil.which(name) is None:
        raise SystemExit(f"required tool is unavailable: {name}")


def main() -> None:
    verify_tool("cargo")
    generate_master_icon()
    generate_icon_variants()
    print(f"generated desktop icon assets in {ICONS_DIR}")


if __name__ == "__main__":
    main()
