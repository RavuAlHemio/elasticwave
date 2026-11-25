#!/usr/bin/env python3
import argparse
import json
import requests


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        dest="token_pair_path",
    )
    args = parser.parse_args()

    with open(args.token_pair_path, "r", encoding="utf-8") as f:
        token_pair = json.load(f)

    response = requests.post(
        "https://wavesuite.example.com/rest-gateway/rest/api/v1/auth/revocation",
        json={},
        headers={
            "Authorization": f"Bearer {token_pair['access_token']}",
        },
    )
    print(response.status_code)
    print(response.text)
    response.raise_for_status()


if __name__ == "__main__":
    main()
