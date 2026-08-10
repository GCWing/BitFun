from __future__ import annotations

import argparse
import io
import json
import math
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any, Sequence

from PIL import Image, ImageDraw, ImageEnhance, ImageOps


DEFAULT_FRAME_DURATION_MS = 40
MAX_IMAGE_BYTES = 16 * 1024 * 1024
MAX_VIDEO_BYTES = 64 * 1024 * 1024
MAX_PREVIEW_BYTES = 4 * 1024 * 1024
MAX_VIDEO_DIMENSION = 4_096
MAX_VIDEO_PIXELS = 9_000_000
VIDEO_SUFFIXES = {".m4v", ".mkv", ".mov", ".mp4", ".webm"}
HEX_COLOR_PATTERN = re.compile(r"^#[0-9a-fA-F]{6}$")
VIDEO_QUALITY_MODES = {"auto", "lossless", "high", "balanced"}
STATIC_QUALITY_MODES = {"auto", "lossless", "quality"}
AUTO_LOSSLESS_PIXEL_SECONDS = 12_000_000


class MediaSupportError(ValueError):
    pass


def parse_crop(value: str) -> tuple[float, float, float, float]:
    try:
        left, top, right, bottom = (float(part.strip()) for part in value.split(","))
    except ValueError as exc:
        raise argparse.ArgumentTypeError("crop must be left,top,right,bottom") from exc
    if not (0 <= left < right <= 1 and 0 <= top < bottom <= 1):
        raise argparse.ArgumentTypeError("crop coordinates must be normalized values in [0, 1]")
    return left, top, right, bottom


def parse_size(value: str) -> tuple[int, int]:
    try:
        width_text, height_text = value.lower().split("x", 1)
        width, height = int(width_text), int(height_text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("size must use WIDTHxHEIGHT") from exc
    if width <= 0 or height <= 0:
        raise argparse.ArgumentTypeError("size values must be positive")
    return width, height


def parse_color(value: str) -> tuple[int, int, int]:
    if not HEX_COLOR_PATTERN.fullmatch(value):
        raise argparse.ArgumentTypeError("color must use #RRGGBB")
    return int(value[1:3], 16), int(value[3:5], 16), int(value[5:7], 16)


def load_image_frames(source: Path, fallback_duration_ms: int) -> tuple[list[Image.Image], list[int], int]:
    image = Image.open(source)
    frame_count = getattr(image, "n_frames", 1)
    frames: list[Image.Image] = []
    durations: list[int] = []
    for frame_index in range(frame_count):
        image.seek(frame_index)
        frames.append(image.convert("RGBA").copy())
        durations.append(image.info.get("duration") or fallback_duration_ms)
    return frames, durations, image.info.get("loop", 0)


def probe_video_duration(source: Path) -> float:
    ffprobe = shutil.which("ffprobe")
    if not ffprobe:
        raise RuntimeError("ffprobe is required to inspect video sources")
    result = subprocess.run(
        [ffprobe, "-v", "error", "-show_entries", "format=duration", "-of", "json", str(source)],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "ffprobe failed to inspect the video source")
    try:
        duration = float(json.loads(result.stdout)["format"]["duration"])
    except (KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
        raise RuntimeError("ffprobe did not report a valid video duration") from exc
    if not math.isfinite(duration) or duration <= 0:
        raise MediaSupportError("Video duration must be a positive finite number")
    return duration


def probe_video(path: Path) -> dict[str, Any]:
    ffprobe = shutil.which("ffprobe")
    if not ffprobe:
        raise RuntimeError("ffprobe is required to inspect the generated background video")
    result = subprocess.run(
        [
            ffprobe,
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            (
                "format=duration,size:"
                "stream=codec_name,width,height,r_frame_rate,pix_fmt,sample_aspect_ratio,display_aspect_ratio"
            ),
            "-of",
            "json",
            str(path),
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=True,
    )
    payload = json.loads(result.stdout)
    stream = payload["streams"][0]
    width = int(stream["width"])
    height = int(stream["height"])
    sample_aspect_ratio = stream.get("sample_aspect_ratio") or "1:1"
    try:
        numerator_text, denominator_text = sample_aspect_ratio.split(":", 1)
        numerator, denominator = int(numerator_text), int(denominator_text)
        if numerator <= 0 or denominator <= 0:
            raise ValueError("sample aspect ratio must be positive")
        pixel_aspect_ratio = numerator / denominator
    except (AttributeError, TypeError, ValueError, ZeroDivisionError):
        pixel_aspect_ratio = 1.0
        sample_aspect_ratio = "1:1"
    display_width = max(1, round(width * pixel_aspect_ratio))
    return {
        "codec": stream["codec_name"],
        "width": width,
        "height": height,
        "displayWidth": display_width,
        "displayHeight": height,
        "sampleAspectRatio": sample_aspect_ratio,
        "displayAspectRatio": stream.get("display_aspect_ratio") or f"{display_width}:{height}",
        "frameRate": stream["r_frame_rate"],
        "pixelFormat": stream["pix_fmt"],
        "durationSeconds": round(float(payload["format"]["duration"]), 3),
        "bytes": int(payload["format"]["size"]),
    }


def validate_host_video(video: dict[str, Any], *, codec: str | None = "vp9") -> None:
    if codec is not None and video["codec"] != codec:
        raise MediaSupportError(f"Background video must use {codec.upper()}, found {video['codec']}")
    display_width = int(video.get("displayWidth", video["width"]))
    display_height = int(video.get("displayHeight", video["height"]))
    if display_width <= 0 or display_height <= 0:
        raise MediaSupportError("Background video display dimensions must be positive")
    if display_width > MAX_VIDEO_DIMENSION or display_height > MAX_VIDEO_DIMENSION:
        raise MediaSupportError(
            f"Background video display dimensions {display_width}x{display_height} "
            f"exceed {MAX_VIDEO_DIMENSION} pixels"
        )
    if display_width * display_height > MAX_VIDEO_PIXELS:
        raise MediaSupportError(
            f"Background video display dimensions {display_width}x{display_height} exceed {MAX_VIDEO_PIXELS} pixels"
        )
    if video["durationSeconds"] > 60:
        raise MediaSupportError("Background video exceeds 60 seconds")
    if video["bytes"] > MAX_VIDEO_BYTES:
        raise MediaSupportError("Background video exceeds 64 MiB")


def extract_video_frame(source: Path, time_seconds: float, *, contact_frame: bool = False) -> Image.Image:
    ffmpeg = shutil.which("ffmpeg")
    if not ffmpeg:
        raise RuntimeError("ffmpeg is required to extract video frames")
    last_error = "ffmpeg failed to extract a frame"
    seek_times = [max(0, time_seconds - offset) for offset in (0, 0.05, 0.1, 0.25)]
    for seek_time in dict.fromkeys(seek_times):
        for fast_seek in (True, False):
            command = [ffmpeg, "-hide_banner", "-loglevel", "error"]
            if fast_seek:
                command.extend(["-ss", f"{seek_time:.6f}"])
            command.extend(["-i", str(source)])
            if not fast_seek:
                command.extend(["-ss", f"{seek_time:.6f}"])
            command.extend(["-map", "0:v:0", "-frames:v", "1"])
            filters = [
                "scale=w='max(2,round(iw*sar/2)*2)':h='max(2,round(ih/2)*2)'",
                "setsar=1",
            ]
            if contact_frame:
                filters.append("scale=640:360:force_original_aspect_ratio=decrease")
            command.extend(["-vf", ",".join(filters)])
            command.extend(["-f", "image2pipe", "-vcodec", "png", "-"])
            result = subprocess.run(command, capture_output=True, check=False)
            if result.returncode != 0:
                last_error = result.stderr.decode("utf-8", errors="replace").strip() or last_error
                continue
            try:
                with Image.open(io.BytesIO(result.stdout)) as image:
                    return image.convert("RGBA").copy()
            except OSError:
                last_error = "ffmpeg returned an invalid frame image"
    raise RuntimeError(last_error)


def load_video_contact_frames(
    source: Path,
    duration: float,
    *,
    frame_count: int = 12,
) -> tuple[list[Image.Image], list[str]]:
    times = [duration * (index + 0.5) / frame_count for index in range(frame_count)]
    return (
        [extract_video_frame(source, time_seconds, contact_frame=True) for time_seconds in times],
        [f"{time_seconds:.2f}s" for time_seconds in times],
    )


def normalized_crop(image: Image.Image, crop: tuple[float, float, float, float]) -> Image.Image:
    width, height = image.size
    left, top, right, bottom = crop
    return image.crop((round(width * left), round(height * top), round(width * right), round(height * bottom)))


def fit_crop(
    image: Image.Image,
    crop: tuple[float, float, float, float],
    size: tuple[int, int],
) -> Image.Image:
    return ImageOps.fit(
        normalized_crop(image, crop),
        size,
        method=Image.Resampling.LANCZOS,
        centering=(0.5, 0.5),
    )


def darken(image: Image.Image, brightness: float, tint: tuple[int, int, int, int]) -> Image.Image:
    darkened = ImageEnhance.Brightness(image.convert("RGBA")).enhance(brightness)
    return Image.alpha_composite(darkened, Image.new("RGBA", darkened.size, tint))


def _encode_vp9_attempt(source: Path, output: Path, *, crf: int | None, lossless: bool) -> None:
    ffmpeg = shutil.which("ffmpeg")
    if not ffmpeg:
        raise RuntimeError("ffmpeg is required to encode the host-managed WebM background")
    max_width = "if(gte(dar,1),3840,2160)"
    max_height = "if(gte(dar,1),2160,3840)"
    scale_factor = f"min(1,min({max_width}/(iw*sar),{max_height}/ih))"
    scale = (
        f"scale=w='max(2,round(iw*sar*{scale_factor}/2)*2)':"
        f"h='max(2,round(ih*{scale_factor}/2)*2)',setsar=1,fps=30"
    )
    command = [
        ffmpeg,
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        str(source),
        "-t",
        "60",
        "-an",
        "-map_metadata",
        "-1",
        "-vf",
        scale,
        "-c:v",
        "libvpx-vp9",
        "-b:v",
        "0",
        "-pix_fmt",
        "yuv420p",
        "-row-mt",
        "1",
        "-deadline",
        "good",
    ]
    if lossless:
        command.extend(["-lossless", "1"])
    else:
        command.extend(["-crf", str(crf)])
    command.append(str(output))
    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "ffmpeg failed to encode the WebM background")


def _source_pixel_seconds(source: Path) -> float | None:
    try:
        info = probe_video(source)
        return (
            min(float(info["durationSeconds"]), 60)
            * int(info["displayWidth"])
            * int(info["displayHeight"])
        )
    except (KeyError, IndexError, OSError, RuntimeError, subprocess.CalledProcessError, json.JSONDecodeError):
        return None


def encode_vp9_webm(
    source: Path,
    output: Path,
    *,
    quality: str = "auto",
    crf: int | None = None,
) -> dict[str, Any]:
    if quality not in VIDEO_QUALITY_MODES:
        raise MediaSupportError(f"Unknown video quality mode: {quality}")
    if crf is not None and not 0 <= crf <= 63:
        raise MediaSupportError("Video CRF must be between 0 and 63")

    attempts: list[dict[str, Any]] = []

    def attempt(*, attempt_crf: int | None, lossless: bool) -> dict[str, Any] | None:
        _encode_vp9_attempt(source, output, crf=attempt_crf, lossless=lossless)
        size = output.stat().st_size
        attempts.append({"codecLossless": lossless, "crf": attempt_crf, "bytes": size})
        if size > MAX_VIDEO_BYTES:
            return None
        video = probe_video(output)
        validate_host_video(video)
        return {
            "policy": "custom-crf" if crf is not None else quality,
            "codecLossless": lossless,
            "crf": attempt_crf,
            "attempts": attempts,
            "hostTransform": {
                "maxDurationSeconds": 60,
                "maxLandscapeSize": [3840, 2160],
                "maxPortraitSize": [2160, 3840],
                "framesPerSecond": 30,
                "pixelFormat": "yuv420p",
                "sampleAspectRatio": "1:1",
                "audio": "removed",
                "metadata": "removed",
            },
            "video": video,
        }

    if crf is not None:
        result = attempt(attempt_crf=crf, lossless=False)
        if result is not None:
            return result
        raise MediaSupportError(
            f"VP9 CRF {crf} produced {output.stat().st_size} bytes, above the {MAX_VIDEO_BYTES}-byte limit"
        )

    if quality == "lossless":
        result = attempt(attempt_crf=None, lossless=True)
        if result is not None:
            return result
        raise MediaSupportError(
            f"Lossless VP9 produced {output.stat().st_size} bytes, above the {MAX_VIDEO_BYTES}-byte limit"
        )

    if quality in {"high", "balanced"}:
        selected_crf = 20 if quality == "high" else 32
        result = attempt(attempt_crf=selected_crf, lossless=False)
        if result is not None:
            return result
        raise MediaSupportError(
            f"Video quality mode {quality} produced {output.stat().st_size} bytes; use auto or a higher --video-crf"
        )

    pixel_seconds = _source_pixel_seconds(source)
    if pixel_seconds is not None and pixel_seconds <= AUTO_LOSSLESS_PIXEL_SECONDS:
        result = attempt(attempt_crf=None, lossless=True)
        if result is not None:
            return result

    selected_crf = 20
    for _ in range(6):
        result = attempt(attempt_crf=selected_crf, lossless=False)
        if result is not None:
            return result
        ratio = max(output.stat().st_size / MAX_VIDEO_BYTES, 1.01)
        step = max(4, math.ceil(6 * math.log2(ratio)))
        next_crf = min(63, selected_crf + step)
        if next_crf == selected_crf:
            break
        selected_crf = next_crf
    raise MediaSupportError(
        f"Automatic VP9 encoding could not satisfy the {MAX_VIDEO_BYTES}-byte limit; attempts: {attempts}"
    )


def save_webp(image: Image.Image, output: Path, quality: int) -> None:
    image.convert("RGB").save(output, format="WEBP", quality=quality, method=6)


def encode_webp(
    image: Image.Image,
    output: Path,
    *,
    mode: str = "auto",
    quality: int | None = None,
    max_bytes: int = MAX_IMAGE_BYTES,
) -> dict[str, Any]:
    if mode not in STATIC_QUALITY_MODES:
        raise MediaSupportError(f"Unknown static quality mode: {mode}")
    if quality is not None and not 1 <= quality <= 100:
        raise MediaSupportError("Static quality must be between 1 and 100")
    attempts: list[dict[str, Any]] = []

    def attempt_lossless() -> dict[str, Any] | None:
        image.convert("RGB").save(output, format="WEBP", lossless=True, method=6)
        size = output.stat().st_size
        attempts.append({"lossless": True, "quality": None, "bytes": size})
        if size <= max_bytes:
            return {"lossless": True, "quality": None, "bytes": size, "attempts": attempts}
        return None

    def attempt_quality(selected_quality: int) -> dict[str, Any] | None:
        save_webp(image, output, selected_quality)
        size = output.stat().st_size
        attempts.append({"lossless": False, "quality": selected_quality, "bytes": size})
        if size <= max_bytes:
            return {"lossless": False, "quality": selected_quality, "bytes": size, "attempts": attempts}
        return None

    if quality is not None:
        result = attempt_quality(quality)
        if result is not None:
            return result
        raise MediaSupportError(f"WebP quality {quality} exceeds the {max_bytes}-byte asset limit")
    if mode == "lossless":
        result = attempt_lossless()
        if result is not None:
            return result
        raise MediaSupportError(f"Lossless WebP exceeds the {max_bytes}-byte asset limit")
    if mode == "quality":
        result = attempt_quality(92)
        if result is not None:
            return result
        raise MediaSupportError(f"WebP quality mode exceeds the {max_bytes}-byte asset limit")

    result = attempt_lossless()
    if result is not None:
        return result
    for selected_quality in (95, 92, 88, 84, 80, 75, 70):
        result = attempt_quality(selected_quality)
        if result is not None:
            return result
    raise MediaSupportError(f"Automatic WebP encoding could not satisfy the {max_bytes}-byte asset limit")


def create_contact_sheet(
    frames: Sequence[Image.Image],
    output: Path,
    *,
    labels: Sequence[str] | None = None,
    columns: int = 4,
    rows: int = 3,
    tile_size: tuple[int, int] = (320, 180),
) -> None:
    if not frames:
        raise MediaSupportError("Contact sheet requires at least one frame")
    tile_count = columns * rows
    indexes = [round(index * (len(frames) - 1) / (tile_count - 1)) for index in range(tile_count)]
    sheet = Image.new("RGB", (columns * tile_size[0], rows * tile_size[1]), "#08111f")
    draw = ImageDraw.Draw(sheet)
    for tile_index, frame_index in enumerate(indexes):
        frame = ImageOps.fit(frames[frame_index].convert("RGB"), tile_size)
        x = (tile_index % columns) * tile_size[0]
        y = (tile_index // columns) * tile_size[1]
        sheet.paste(frame, (x, y))
        label = labels[frame_index] if labels else f"frame {frame_index}"
        draw.rectangle((x + 8, y + 8, x + 94, y + 30), fill=(4, 10, 20))
        draw.text((x + 14, y + 11), label, fill=(225, 240, 248))
    sheet.save(output, format="PNG", optimize=True)


def create_asset_preview_sheet(
    assets: Path,
    output: Path,
    names: Sequence[str],
    *,
    columns: int = 3,
) -> None:
    tile_width, tile_height = 360, 250
    content_size = (332, 202)
    rows = math.ceil(len(names) / columns)
    sheet = Image.new("RGB", (tile_width * columns, tile_height * rows), "#08111f")
    draw = ImageDraw.Draw(sheet)
    for index, name in enumerate(names):
        path = assets / name
        if not path.is_file():
            raise FileNotFoundError(f"Missing preview asset: {path}")
        with Image.open(path) as image:
            preview = ImageOps.contain(image.convert("RGB"), content_size, Image.Resampling.LANCZOS)
        column, row = index % columns, index // columns
        tile_x, tile_y = column * tile_width, row * tile_height
        image_x = tile_x + (tile_width - preview.width) // 2
        image_y = tile_y + 38 + (content_size[1] - preview.height) // 2
        sheet.paste(preview, (image_x, image_y))
        draw.text((tile_x + 14, tile_y + 12), name, fill=(225, 240, 248))
    sheet.save(output, format="PNG", optimize=True)


def inspect_assets(assets: Path) -> dict[str, object]:
    result: dict[str, object] = {}
    for path in sorted(assets.glob("*.webp")):
        with Image.open(path) as image:
            result[path.name] = {
                "bytes": path.stat().st_size,
                "size": list(image.size),
                "frames": getattr(image, "n_frames", 1),
                "withinAssetLimit": path.stat().st_size <= MAX_IMAGE_BYTES,
            }
    for path in sorted(assets.glob("*.webm")):
        result[path.name] = {
            "bytes": path.stat().st_size,
            "withinAssetLimit": path.stat().st_size <= MAX_VIDEO_BYTES,
        }
    return result
