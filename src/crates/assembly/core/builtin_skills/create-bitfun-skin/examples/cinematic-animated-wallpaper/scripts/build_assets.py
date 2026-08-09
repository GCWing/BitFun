from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from PIL import ImageFilter


SKILL_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(SKILL_ROOT / "scripts"))

import media_support as media  # noqa: E402


PREVIEW_ASSET_NAMES = (
    "preview.webp",
    "floating.webp",
    "sidebar.webp",
    "card-detail.webp",
    "card-portrait.webp",
    "dialog.webp",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build cinematic animated-wallpaper skin assets.")
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="Skin root containing package/assets.")
    parser.add_argument("--frame-index", type=int, default=-1, help="Still frame; -1 selects the midpoint.")
    parser.add_argument("--frame-time", type=float, help="Still time in seconds for video sources; defaults to midpoint.")
    parser.add_argument("--frame-duration-ms", type=int, default=media.DEFAULT_FRAME_DURATION_MS)
    parser.add_argument("--video-quality", choices=sorted(media.VIDEO_QUALITY_MODES), default="auto")
    parser.add_argument("--video-crf", type=int, help="Explicit CRF override for reproducibility or manual tuning.")
    parser.add_argument("--static-quality-mode", choices=sorted(media.STATIC_QUALITY_MODES), default="auto")
    parser.add_argument("--static-quality", type=int, help="Explicit lossy WebP quality override.")
    parser.add_argument("--tint-color", type=media.parse_color, default=media.parse_color("#050f1d"))
    parser.add_argument("--contact-only", action="store_true")
    parser.add_argument("--static-only", action="store_true", help="Build derived stills without encoding background.webm.")
    parser.add_argument("--floating-crop", type=media.parse_crop, default=media.parse_crop("0.18,0,0.82,1"))
    parser.add_argument("--floating-size", type=media.parse_size, default=media.parse_size("820x720"))
    parser.add_argument("--sidebar-crop", type=media.parse_crop, default=media.parse_crop("0.64,0,1,1"))
    parser.add_argument("--sidebar-size", type=media.parse_size, default=media.parse_size("460x720"))
    parser.add_argument("--detail-crop", type=media.parse_crop, default=media.parse_crop("0.41,0.54,1,1"))
    parser.add_argument("--detail-size", type=media.parse_size, default=media.parse_size("760x330"))
    parser.add_argument("--portrait-crop", type=media.parse_crop, default=media.parse_crop("0.21,0.06,0.80,0.79"))
    parser.add_argument("--portrait-size", type=media.parse_size, default=media.parse_size("760x525"))
    parser.add_argument("--dialog-crop", type=media.parse_crop, default=media.parse_crop("0.09,0.28,0.91,1"))
    parser.add_argument("--dialog-size", type=media.parse_size, default=media.parse_size("1040x515"))
    return parser.parse_args()


def build(args: argparse.Namespace) -> None:
    source = args.source.resolve()
    output = args.output.resolve()
    if not source.is_file():
        raise FileNotFoundError(f"Source image does not exist: {source}")
    if args.video_crf is not None and not 0 <= args.video_crf <= 63:
        raise ValueError("video CRF must be between 0 and 63")
    if args.static_quality is not None and not 1 <= args.static_quality <= 100:
        raise ValueError("static quality must be between 1 and 100")
    if args.frame_duration_ms <= 0:
        raise ValueError("frame duration must be positive")

    is_video = source.suffix.lower() in media.VIDEO_SUFFIXES
    duration = media.probe_video_duration(source) if is_video else None
    if is_video:
        frames, labels = media.load_video_contact_frames(source, duration)
    else:
        frames, _, _ = media.load_image_frames(source, args.frame_duration_ms)
        labels = None

    output.mkdir(parents=True, exist_ok=True)
    contact_sheet = output / "contact-sheet.png"
    media.create_contact_sheet(frames, contact_sheet, labels=labels)
    if args.contact_only:
        print(json.dumps({
            "contactSheet": str(contact_sheet),
            "frames": len(frames),
            "durationSeconds": duration,
        }, indent=2))
        return

    assets = output / "package" / "assets"
    assets.mkdir(parents=True, exist_ok=True)
    video_encoding = None
    if not args.static_only:
        video_encoding = media.encode_vp9_webm(
            source,
            assets / "background.webm",
            quality=args.video_quality,
            crf=args.video_crf,
        )

    if is_video:
        frame_time = duration / 2 if args.frame_time is None else args.frame_time
        if not 0 <= frame_time <= duration:
            raise ValueError(f"frame time {frame_time} is outside 0..{duration}")
        still = media.extract_video_frame(source, frame_time)
        selected_frame: int | float = frame_time
    else:
        if args.frame_time is not None:
            raise ValueError("--frame-time is supported only for video sources")
        frame_index = len(frames) // 2 if args.frame_index < 0 else args.frame_index
        if frame_index >= len(frames):
            raise ValueError(f"frame index {frame_index} is outside 0..{len(frames) - 1}")
        still = frames[frame_index]
        selected_frame = frame_index

    tint_red, tint_green, tint_blue = args.tint_color
    tint = (tint_red, tint_green, tint_blue)

    floating = media.fit_crop(still, args.floating_crop, args.floating_size)
    floating = media.darken(floating, 0.58, (*tint, 36))
    static_encoding: dict[str, object] = {}

    def encode_asset(name: str, image, *, preview: bool = False) -> None:
        static_encoding[name] = media.encode_webp(
            image,
            assets / name,
            mode=args.static_quality_mode,
            quality=args.static_quality,
            max_bytes=media.MAX_PREVIEW_BYTES if preview else media.MAX_IMAGE_BYTES,
        )

    encode_asset("floating.webp", floating)

    sidebar = media.fit_crop(still, args.sidebar_crop, args.sidebar_size)
    sidebar = media.darken(sidebar, 0.62, (*tint, 28))
    encode_asset("sidebar.webp", sidebar)

    detail = media.fit_crop(still, args.detail_crop, args.detail_size)
    detail = media.darken(detail, 0.42, (*tint, 72)).filter(ImageFilter.GaussianBlur(0.35))
    encode_asset("card-detail.webp", detail)

    portrait = media.fit_crop(still, args.portrait_crop, args.portrait_size)
    portrait = media.darken(portrait, 0.34, (*tint, 92)).filter(ImageFilter.GaussianBlur(0.55))
    encode_asset("card-portrait.webp", portrait)

    dialog = media.fit_crop(still, args.dialog_crop, args.dialog_size)
    dialog = media.darken(dialog, 0.38, (*tint, 90)).filter(ImageFilter.GaussianBlur(0.45))
    encode_asset("dialog.webp", dialog)

    encode_asset("preview.webp", still, preview=True)
    asset_preview_sheet = output / "asset-preview-sheet.png"
    media.create_asset_preview_sheet(assets, asset_preview_sheet, PREVIEW_ASSET_NAMES)
    print(json.dumps({
        "source": str(source),
        "selectedFrame": selected_frame,
        "selectedFrameKind": "seconds" if is_video else "index",
        "tintColor": "#{:02x}{:02x}{:02x}".format(*args.tint_color),
        "assetPreviewSheet": str(asset_preview_sheet),
        "encoding": {
            "video": video_encoding,
            "static": static_encoding,
        },
        "assets": media.inspect_assets(assets),
    }, indent=2))


if __name__ == "__main__":
    build(parse_args())
