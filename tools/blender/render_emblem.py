"""Renders Shuusshy's animated emblem with Blender (Cycles) and post-processes it with numpy.

The emblem is a 3D take on the logo: five rounded bars on a dark glass tile. The centre bar is
the voice, the outer bars are noise. While noise cancellation is on, the voice bar "speaks"
(pulses), the noise bars stay shushed, and a soft ripple ring radiates from the centre.

Outputs (all PNG, transparent background):
  src/assets/emblem-on.png    horizontal strip of FRAMES banner frames, 2x for HiDPI
  src/assets/emblem-off.png   one static frame: everything dimmed (noise cancellation off)
  src-tauri/icons/tray/on-NN.png   32x32 tray animation frames (seamless loop)

Run from the repository root (needs Blender 4.2+; tested with 5.3):

    blender -b --factory-startup -P tools/blender/render_emblem.py -- .
    blender -b --factory-startup -P tools/blender/render_emblem.py -- . --icon   # app icon
"""

import math
import os
import sys
import tempfile

import bpy
import numpy as np

ROOT = os.path.abspath(sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else ".")
FRAMES = 36  # banner loop: 36 frames at 18 fps = 2 s
TRAY_FRAMES = 12  # tray loop: 12 frames at ~6 fps = 2 s
BANNER_PX = 112  # rendered size of one banner frame (shown at 56 CSS px)
TRAY_RENDER_PX = 128  # tray frames are rendered large and box-filtered down to 32 px

CYAN = (0.20, 0.85, 1.00)
TEAL = (0.15, 0.62, 0.85)
DIM = (0.30, 0.36, 0.44)


# --------------------------------------------------------------------------- scene setup


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    s = bpy.context.scene
    s.render.engine = "CYCLES"
    s.cycles.device = "CPU"
    s.cycles.samples = 48
    s.cycles.use_denoising = True
    s.render.film_transparent = True
    s.render.image_settings.file_format = "PNG"
    s.render.image_settings.color_mode = "RGBA"
    s.view_settings.view_transform = "Standard"
    s.world = bpy.data.worlds.new("w")
    s.world.use_nodes = True
    bg = s.world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (0.01, 0.015, 0.025, 1)
    bg.inputs["Strength"].default_value = 0.4
    return s


def material(name, base, metallic=0.0, rough=0.4, emit=(0, 0, 0), strength=0.0, alpha=1.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = m.node_tree.nodes["Principled BSDF"]
    p.inputs["Base Color"].default_value = (*base, 1)
    p.inputs["Metallic"].default_value = metallic
    p.inputs["Roughness"].default_value = rough
    p.inputs["Emission Color"].default_value = (*emit, 1)
    p.inputs["Emission Strength"].default_value = strength
    p.inputs["Alpha"].default_value = alpha
    return m


def bevelled_box(name, size, loc, bevel, mat, corner=0.0):
    """Box with rounded edges. `corner` > 0 additionally rounds the four vertical edges with a
    larger radius (a rounded rectangle in plan view), without the overlapping geometry a single
    large bevel on a thin slab would create."""
    # Built at the origin and moved afterwards: applying the scale here would otherwise also
    # bake the location into the mesh, doubling it once the object is placed.
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0))
    o = bpy.context.object
    o.name = name
    o.scale = size
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    o.location = loc
    if corner > 0:
        me = o.data
        attr = me.attributes.new("bevel_weight_edge", "FLOAT", "EDGE")
        for e in me.edges:
            a, b = (me.vertices[i].co for i in e.vertices)
            vertical = abs(a.x - b.x) < 1e-6 and abs(a.y - b.y) < 1e-6
            attr.data[e.index].value = 1.0 if vertical else 0.0
        c = o.modifiers.new("corners", "BEVEL")
        c.width = corner
        c.segments = 12
        c.limit_method = "WEIGHT"
    b = o.modifiers.new("bevel", "BEVEL")
    b.width = bevel
    b.segments = 6
    b.limit_method = "ANGLE" if corner > 0 else "NONE"
    b.use_clamp_overlap = True
    o.data.materials.append(mat)
    bpy.ops.object.shade_smooth()
    return o


def build(top_down: bool):
    """Tile, five bars, ripple ring, lights and camera. Returns the animated parts."""
    s = reset()
    tile_mat = material("tile", (0.03, 0.04, 0.055), metallic=0.35, rough=0.42)
    rim_mat = material("rim", (0.02, 0.05, 0.07), emit=CYAN, strength=1.5)
    bevelled_box("tile", (2.0, 2.0, 0.22), (0, 0, -0.11), 0.05, tile_mat, corner=0.42)
    # Thin glowing rim just under the tile edge: the "tech" outline.
    bevelled_box("rim", (2.05, 2.05, 0.04), (0, 0, -0.2), 0.015, rim_mat, corner=0.44)

    xs = [-0.64, -0.32, 0.0, 0.32, 0.64]
    bars, bar_mats = [], []
    for i, x in enumerate(xs):
        m = material(f"bar{i}", (0.05, 0.08, 0.1), rough=0.25, emit=CYAN, strength=4)
        o = bevelled_box(f"bar{i}", (0.17, 0.17, 1.0), (x, 0, 0.0), 0.084, m)
        # Bars stand on the tile and grow along +Y (the tile's "up" in the picture).
        o.rotation_euler = (math.radians(90), 0, 0)
        o.location = (x, 0, 0.09)
        bars.append(o)
        bar_mats.append(m)

    bpy.ops.mesh.primitive_torus_add(
        major_radius=0.5, minor_radius=0.025, major_segments=96, minor_segments=12, location=(0, 0, 0.1)
    )
    ring = bpy.context.object
    ring_mat = material("ring", (0, 0, 0), emit=CYAN, strength=6)
    ring.data.materials.append(ring_mat)

    # Lights: soft key from the top left, cyan rim from behind.
    bpy.ops.object.light_add(type="AREA", location=(-2.5, -2.5, 4))
    key = bpy.context.object
    key.data.energy = 260
    key.data.size = 3
    key.rotation_euler = (math.radians(35), 0, math.radians(-45))
    bpy.ops.object.light_add(type="AREA", location=(0, 3, 1.5))
    rim = bpy.context.object
    rim.data.energy = 50
    rim.data.color = CYAN
    rim.data.size = 2
    rim.rotation_euler = (math.radians(-60), 0, 0)

    bpy.ops.object.camera_add()
    cam = bpy.context.object
    s.camera = cam
    cam.data.type = "ORTHO"
    if top_down:
        cam.location = (0, 0, 6)
        cam.rotation_euler = (0, 0, 0)
        cam.data.ortho_scale = 2.25
    else:
        # Slight tilt so the bars read as 3D in the banner.
        cam.location = (0, -4.2, 4.6)
        cam.rotation_euler = (math.radians(42), 0, 0)
        cam.data.ortho_scale = 2.9
    return s, bars, bar_mats, ring, ring_mat


# --------------------------------------------------------------------------- animation


def set_bar_length(o, length):
    """Moves the bar's end vertices (before the bevel modifier) so the rounded ends keep their
    shape; scaling the object would squash them."""
    for v in o.data.vertices:
        v.co.z = math.copysign(length / 2, v.co.z)
    o.data.update()


def pose(t: float, on: bool, bars, bar_mats, ring, ring_mat):
    """Sets the emblem for loop phase t in [0, 1). Every term is periodic, so the loop is seamless."""
    w = 2 * math.pi * t
    if on:
        voice = 0.72 + 0.2 * math.sin(w) + 0.1 * math.sin(2 * w + 1.3) + 0.05 * math.sin(3 * w + 0.4)
        side = 0.45 + 0.12 * math.sin(w + 0.9) + 0.05 * math.sin(3 * w + 2.0)
        noise = 0.16 + 0.025 * math.sin(4 * w)  # shushed: tiny and calm
        heights = [noise, side, voice, side * 0.97, noise]
        strengths = [0.8, 3.0, 6.0, 3.0, 0.8]
        colors = [TEAL, CYAN, CYAN, CYAN, TEAL]
    else:
        heights = [0.2, 0.42, 0.6, 0.42, 0.2]
        strengths = [0.0] * 5
        colors = [DIM] * 5
    for o, h, m, st, c in zip(bars, heights, bar_mats, strengths, colors):
        set_bar_length(o, 0.17 + 1.45 * h)  # centred: grows both ways like the 2D logo
        p = m.node_tree.nodes["Principled BSDF"]
        p.inputs["Emission Strength"].default_value = st
        p.inputs["Emission Color"].default_value = (*c, 1)
        p.inputs["Base Color"].default_value = (*([0.05, 0.08, 0.1] if on else [0.16, 0.18, 0.21]), 1)
    # Ripple: expands from the voice bar and fades out once per loop.
    if on:
        r = 0.35 + 0.75 * t
        ring.scale = (r / 0.5, r / 0.5, 1)
        fade = (1 - t) ** 1.6
        ring_mat.node_tree.nodes["Principled BSDF"].inputs["Emission Strength"].default_value = 7 * fade
        ring.hide_render = fade < 0.02
    else:
        ring.hide_render = True


# --------------------------------------------------------------------------- image post


def render_to_array(s, px, path):
    s.render.resolution_x = px
    s.render.resolution_y = px
    s.render.filepath = path
    bpy.ops.render.render(write_still=True)
    img = bpy.data.images.load(path)
    a = np.array(img.pixels[:], dtype=np.float32).reshape(px, px, 4)
    bpy.data.images.remove(img)
    return a


def gaussian_blur(a, sigma):
    r = int(sigma * 3)
    k = np.exp(-0.5 * (np.arange(-r, r + 1) / sigma) ** 2)
    k /= k.sum()
    out = a
    for axis in (0, 1):
        out = np.apply_along_axis(lambda v: np.convolve(v, k, mode="same"), axis, out)
    return out


def bloom(a, sigma, amount):
    """Adds a soft glow around bright (emissive) pixels. a: straight-alpha RGBA."""
    rgb, alpha = a[..., :3], a[..., 3:4]
    pre = rgb * alpha
    bright = np.clip(pre - 0.55, 0, None)
    glow = np.stack([gaussian_blur(bright[..., c], sigma) for c in range(3)], -1) * amount
    pre = pre + glow
    alpha2 = np.clip(np.maximum(alpha[..., 0], glow.max(-1) * 1.4), 0, 1)[..., None]
    rgb2 = np.clip(pre / np.maximum(alpha2, 1e-4), 0, 1)
    return np.concatenate([rgb2, alpha2], -1)


def downscale(a, f):
    """Box filter on premultiplied colour (keeps edges clean on the taskbar)."""
    h, w = a.shape[0] // f, a.shape[1] // f
    pre = np.concatenate([a[..., :3] * a[..., 3:4], a[..., 3:4]], -1)
    pre = pre.reshape(h, f, w, f, 4).mean(axis=(1, 3))
    alpha = pre[..., 3:4]
    return np.concatenate([np.clip(pre[..., :3] / np.maximum(alpha, 1e-4), 0, 1), alpha], -1)


def save(a, path):
    h, w = a.shape[:2]
    img = bpy.data.images.new(os.path.basename(path), w, h, alpha=True)
    img.pixels[:] = np.clip(a, 0, 1).ravel()
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)
    print("wrote", path)


# --------------------------------------------------------------------------- main


def main():
    tmp = tempfile.mkdtemp(prefix="shuusshy-emblem-")
    assets = os.path.join(ROOT, "src", "assets")
    tray_dir = os.path.join(ROOT, "src-tauri", "icons", "tray")
    os.makedirs(assets, exist_ok=True)
    os.makedirs(tray_dir, exist_ok=True)

    # Banner: tilted 3D view.
    s, bars, mats, ring, ring_mat = build(top_down=False)
    frames = []
    for i in range(FRAMES):
        pose(i / FRAMES, True, bars, mats, ring, ring_mat)
        a = render_to_array(s, BANNER_PX, os.path.join(tmp, f"b{i:02d}.png"))
        frames.append(bloom(a, 3.0, 1.3))
    # Strip is stored left to right; Blender images are bottom-up, so rows stay consistent.
    save(np.concatenate(frames, axis=1), os.path.join(assets, "emblem-on.png"))
    pose(0, False, bars, mats, ring, ring_mat)
    off = render_to_array(s, BANNER_PX, os.path.join(tmp, "off.png"))
    save(off, os.path.join(assets, "emblem-off.png"))

    # Tray: top-down so the emblem fills the small icon.
    s, bars, mats, ring, ring_mat = build(top_down=True)
    s.cycles.samples = 32
    for i in range(TRAY_FRAMES):
        pose(i / TRAY_FRAMES, True, bars, mats, ring, ring_mat)
        a = render_to_array(s, TRAY_RENDER_PX, os.path.join(tmp, f"t{i:02d}.png"))
        a = bloom(a, 4.0, 1.0)
        save(downscale(a, TRAY_RENDER_PX // 32), os.path.join(tray_dir, f"on-{i:02d}.png"))


def render_icon():
    """1024 px app icon (taskbar, Start menu, installer): the emblem lit, without the ripple.
    Turned into every icon size with `npm run tauri icon src-tauri/icons/source/app-icon.png`."""
    out = os.path.join(ROOT, "src-tauri", "icons", "source")
    os.makedirs(out, exist_ok=True)
    s, bars, mats, ring, ring_mat = build(top_down=True)
    s.camera.data.ortho_scale = 2.32
    s.cycles.samples = 64
    pose(0.15, True, bars, mats, ring, ring_mat)
    ring.hide_render = True
    a = render_to_array(s, 1024, os.path.join(tempfile.mkdtemp(), "icon.png"))
    save(bloom(a, 18.0, 0.9), os.path.join(out, "app-icon.png"))


if "--icon" in sys.argv:
    render_icon()
else:
    main()
