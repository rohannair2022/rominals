#!/usr/bin/env python3
"""Stream token output from a local mlx-lm model to the terminal."""

from __future__ import annotations

import argparse
import sys

from mlx_lm import load, stream_generate
from mlx_lm.sample_utils import make_sampler

DEFAULT_MODEL = "mlx-community/Qwen3.5-4B-MLX-4bit"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Stream mlx-lm generation token-by-token in your terminal."
    )
    parser.add_argument(
        "--model",
        default=DEFAULT_MODEL,
        help=f"Hugging Face model repo or local path (default: {DEFAULT_MODEL})",
    )
    parser.add_argument(
        "--prompt",
        default="Give me a 5-line thesis on AAPL with one risk and one catalyst.",
        help="Prompt to generate from.",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=300,
        help="Maximum number of generated tokens.",
    )
    parser.add_argument(
        "--temp",
        type=float,
        default=0.2,
        help="Sampling temperature.",
    )
    parser.add_argument(
        "--top-p",
        type=float,
        default=0.95,
        help="Top-p nucleus sampling value.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    print(f"Loading model: {args.model}", file=sys.stderr, flush=True)
    model, tokenizer = load(args.model)
    print("Streaming output:\n", file=sys.stderr, flush=True)

    try:
        sampler = make_sampler(temp=args.temp, top_p=args.top_p)

        for chunk in stream_generate(
            model,
            tokenizer,
            prompt=args.prompt,
            max_tokens=args.max_tokens,
            sampler=sampler,
        ):
            if chunk.text:
                print(chunk.text, end="", flush=True)
    except KeyboardInterrupt:
        print("\n\nInterrupted.", file=sys.stderr, flush=True)
        return 130

    print("", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
