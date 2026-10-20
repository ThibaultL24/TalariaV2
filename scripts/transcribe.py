#!/usr/bin/env python3
"""Transcribe WhatsApp voice notes and video audio tracks to French text."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import whisper


def extract_audio_wav(media: Path, wav: Path) -> None:
    cmd = [
        "ffmpeg",
        "-y",
        "-i",
        str(media),
        "-vn",
        "-ac",
        "1",
        "-ar",
        "16000",
        "-f",
        "wav",
        str(wav),
    ]
    subprocess.run(cmd, check=True, capture_output=True)


def transcribe_file(model: whisper.Whisper, media: Path) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        wav = Path(tmp) / "audio.wav"
        extract_audio_wav(media, wav)
        result = model.transcribe(str(wav), language="fr", task="transcribe", verbose=False)
        parts: list[dict] = []
        full: list[str] = []
        for seg in result.get("segments") or []:
            text = (seg.get("text") or "").strip()
            if not text:
                continue
            full.append(text)
            parts.append(
                {
                    "start": round(float(seg.get("start", 0)), 2),
                    "end": round(float(seg.get("end", 0)), 2),
                    "text": text,
                }
            )
        return {
            "source": str(media),
            "language": result.get("language", "fr"),
            "duration": round(float(parts[-1]["end"]) if parts else 0, 2),
            "text": " ".join(full),
            "segments": parts,
        }


def main() -> int:
    parser = argparse.ArgumentParser(description="Transcribe media files (French).")
    parser.add_argument(
        "files",
        nargs="+",
        type=Path,
        help="Audio/video paths to transcribe",
    )
    parser.add_argument(
        "--model",
        default="small",
        help="faster-whisper model size (default: small)",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path("transcriptions.json"),
        help="JSON output path",
    )
    args = parser.parse_args()

    model = whisper.load_model(args.model)
    results: list[dict] = []
    for path in args.files:
        if not path.is_file():
            print(f"skip (missing): {path}", file=sys.stderr)
            continue
        print(f"transcribing: {path.name}", file=sys.stderr)
        results.append(transcribe_file(model, path))

    args.output.write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8")
    for r in results:
        name = Path(r["source"]).name
        print(f"\n=== {name} ===\n{r['text']}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
