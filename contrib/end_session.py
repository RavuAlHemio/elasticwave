#!/usr/bin/env python3
import argparse
import json
import tomllib

import requests

from urllib.parse import urljoin


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        dest="token_pair_path",
    )
    args = parser.parse_args()

    with open(args.token_pair_path, "r", encoding="utf-8") as f:
        token_pair = json.load(f)

    import http.client
    http.client.HTTPConnection.debuglevel = 1

    with open("config.toml", "rb") as f:
        config = tomllib.load(f)

    revoke_url = urljoin(config["wave_suite"]["base_url"], "rest-gateway/rest/api/v1/auth/revocation")
    username = config["wave_suite"]["username"]
    password = config["wave_suite"]["password"]

    response = requests.post(
        revoke_url,
        data={
            "token": token_pair["access_token"],
        },
        auth=(username, password),
        verify=False,
    )
    print(response.status_code)
    print(response.text)
    response.raise_for_status()


if __name__ == "__main__":
    main()
