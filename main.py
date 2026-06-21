#!/usr/bin/env python3
"""
Tiny CLI: `python main.py AAPL` -> prints a quote snapshot from Yahoo Finance.

Uses the public v8 chart endpoint, which returns a `meta` object with the
fields we care about and does NOT require the cookie/crumb auth that the
newer v7 quote endpoint now demands.
"""

import sys
import json
import requests
from typing import Optional


def money(v: Optional[float]) -> str:
    """Format an Optional[float] as a price string, or 'n/a' if absent."""
    return f"{v:.2f}" if v is not None else "n/a"


def main():
    # 1. Read the ticker from argv.
    if len(sys.argv) < 2:
        print("Usage: python main.py <TICKER>   (e.g. AAPL, MSFT, KRKNF)", file=sys.stderr)
        sys.exit(2)
    
    ticker = sys.argv[1].upper()
    url = f"https://query1.finance.yahoo.com/v8/finance/chart/{ticker}"

    # 2. Build a request with a User-Agent. Yahoo returns 429/403 without one.
    headers = {
        "User-Agent": "Mozilla/5.0 (python-yfinance/0.1)"
    }

    # 3. Fire the request.
    try:
        resp = requests.get(url, headers=headers, timeout=10)
        resp.raise_for_status()
    except requests.RequestException as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

    # 4. Parse JSON.
    try:
        body = resp.json()
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON: {e}", file=sys.stderr)
        sys.exit(1)

    # 5. Extract the meta object.
    try:
        result = body.get("chart", {}).get("result")
        if not result or len(result) == 0:
            print("No data — is that a valid ticker?", file=sys.stderr)
            sys.exit(1)
        
        meta = result[0].get("meta", {})
    except (KeyError, IndexError, TypeError) as e:
        print(f"Error parsing response: {e}", file=sys.stderr)
        sys.exit(1)

    # 6. Extract fields (use .get() for optional values).
    symbol = meta.get("symbol", "")
    currency = meta.get("currency")
    full_exchange_name = meta.get("fullExchangeName")
    long_name = meta.get("longName")
    short_name = meta.get("shortName")
    regular_market_price = meta.get("regularMarketPrice")
    chart_previous_close = meta.get("chartPreviousClose")
    regular_market_day_high = meta.get("regularMarketDayHigh")
    regular_market_day_low = meta.get("regularMarketDayLow")
    regular_market_volume = meta.get("regularMarketVolume")
    fifty_two_week_high = meta.get("fiftyTwoWeekHigh")
    fifty_two_week_low = meta.get("fiftyTwoWeekLow")

    # 7. Calculate change and percentage.
    change = None
    pct = None
    if regular_market_price is not None and chart_previous_close is not None and chart_previous_close != 0:
        change = regular_market_price - chart_previous_close
        pct = (change / chart_previous_close) * 100

    # 8. Print output.
    name = long_name or short_name or symbol

    print(f"{name} ({symbol})")
    print(f"  Exchange:       {full_exchange_name or 'n/a'}")
    print(f"  Currency:       {currency or 'n/a'}")
    print(f"  Price:          {money(regular_market_price)}")
    
    if change is not None and pct is not None:
        sign = "+" if change >= 0 else ""
        print(f"  Change:         {sign}{change:.2f} ({sign}{pct:.2f}%)")
    else:
        print("  Change:         n/a")
    
    print(f"  Prev close:     {money(chart_previous_close)}")
    print(f"  Day range:      {money(regular_market_day_low)} - {money(regular_market_day_high)}")
    print(f"  52-week range:  {money(fifty_two_week_low)} - {money(fifty_two_week_high)}")
    
    volume_str = str(regular_market_volume) if regular_market_volume is not None else "n/a"
    print(f"  Volume:         {volume_str}")


if __name__ == "__main__":
    main()
