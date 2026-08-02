#!/usr/bin/env python3
"""Check whether a local MLX model can read a URL directly."""

from __future__ import annotations

import argparse
import re
import ssl
import sys
import urllib.request
from html.parser import HTMLParser

from mlx_lm import load, stream_generate
from mlx_lm.sample_utils import make_sampler

DEFAULT_MODEL = "mlx-community/Qwen3.5-4B-MLX-4bit"


class _HtmlTextExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self._parts: list[str] = []

    def handle_data(self, data: str) -> None:
        if data and not data.isspace():
            self._parts.append(data)

    def text(self) -> str:
        raw = " ".join(self._parts)
        return re.sub(r"\s+", " ", raw).strip()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run two prompts to test URL-reading behavior in a local MLX model."
    )
    parser.add_argument(
        "--model",
        default=DEFAULT_MODEL,
        help=f"Model repo/path (default: {DEFAULT_MODEL})",
    )
    parser.add_argument(
        "--url",
        default="https://example.com",
        help="URL to test.",
    )
    parser.add_argument(
        "--question",
        default="What is this page for?",
        help="Question to ask about the URL/page.",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=180,
        help="Max generated tokens per run.",
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
        help="Top-p nucleus sampling.",
    )
    parser.add_argument(
        "--max-page-chars",
        type=int,
        default=6000,
        help="Max page text chars to include in the second prompt.",
    )
    parser.add_argument(
        "--ca-bundle",
        default=None,
        help="Optional path to a CA bundle PEM file.",
    )
    parser.add_argument(
        "--insecure",
        action="store_true",
        help="Disable TLS certificate verification for URL fetch (testing only).",
    )
    return parser.parse_args()


def generate(
    model,
    tokenizer,
    prompt: str,
    max_tokens: int,
    temp: float,
    top_p: float,
) -> str:
    sampler = make_sampler(temp=temp, top_p=top_p)
    chunks: list[str] = []
    for chunk in stream_generate(
        model,
        tokenizer,
        prompt=prompt,
        max_tokens=max_tokens,
        sampler=sampler,
    ):
        if chunk.text:
            chunks.append(chunk.text)
    return "".join(chunks).strip()


def build_ssl_context(ca_bundle: str | None, insecure: bool) -> ssl.SSLContext:
    if insecure:
        return ssl._create_unverified_context()

    if ca_bundle:
        return ssl.create_default_context(cafile=ca_bundle)

    try:
        import certifi

        return ssl.create_default_context(cafile=certifi.where())
    except Exception:
        return ssl.create_default_context()


def fetch_page_text(
    url: str, max_chars: int, ca_bundle: str | None, insecure: bool
) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    ssl_context = build_ssl_context(ca_bundle, insecure)
    with urllib.request.urlopen(req, timeout=20, context=ssl_context) as resp:
        html = resp.read().decode("utf-8", errors="ignore")
    parser = _HtmlTextExtractor()
    parser.feed(html)
    text = parser.text()
    return text[:max_chars]


def main() -> int:
    args = parse_args()

    print(f"Loading model: {args.model}", file=sys.stderr)
    model, tokenizer = load(args.model)

    prompt_url_only = (
        "You do not have browsing tools unless explicitly provided. "
        f"Read this URL and answer the question.\nURL: {args.url}\n"
        f"Question: {args.question}\n"
        "If you cannot directly open URLs, reply exactly with: CANNOT_OPEN_LINK"
    )

    print("\n=== Test 1: URL-only prompt ===")
    out1 = generate(
        model,
        tokenizer,
        prompt_url_only,
        args.max_tokens,
        args.temp,
        args.top_p,
    )
    print(out1 or "<empty>")

    print("\nFetching URL content for test 2...", file=sys.stderr)
    page_text = fetch_page_text(
        args.url,
        args.max_page_chars,
        args.ca_bundle,
        args.insecure,
    )

    prompt_with_text = (
        f"Use only the provided extracted page text from {args.url}.\n"
        f"Question: {args.question}\n\n"
        "Extracted page text:\n"
        f"{page_text}\n\n"
        "Answer in 2-4 short lines."
    )

    print("\n=== Test 2: Prompt with fetched page text ===")
    out2 = generate(
        model,
        tokenizer,
        prompt_with_text,
        args.max_tokens,
        args.temp,
        args.top_p,
    )
    print(out2 or "<empty>")

    print("\n=== Interpretation ===")
    if "CANNOT_OPEN_LINK" in out1:
        print("Model cannot open links by itself. It can answer when page text is provided.")
    else:
        print(
            "Model did not explicitly refuse link access. Compare Test 1 vs Test 2 for accuracy."
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
