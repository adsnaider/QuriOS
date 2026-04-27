#! /usr/bin/env nix
#! nix develop --command python

import ast
import base64
import datetime
import os
import re
import sys
import time
from pathlib import Path

import requests
import toml

MAX_TRIES = 3
RETRY_DELAY = 3.0
SYNC_MAX_UPDATE = 32

MIN_STABLE_VERSION = "1.29.0"
MIN_BETA_DATE = MIN_NIGHTLY_DATE = datetime.date.fromisoformat("2018-09-13")

DIST_ROOT = "https://static.rust-lang.org/dist"
MANIFEST_TMP_PATH = Path("manifest.tmp")
TARGETS_PATH = Path("manifests/targets.bzl")
RENAMES_PATH = Path("manifests/renames.bzl")
PROFILES_PATH = Path("manifests/profiles.bzl")

RE_STABLE_VERSION = re.compile(r"^\d+\.\d+\.\d+$")

GITHUB_TOKEN_HEADERS = {}
if "GITHUB_TOKEN" in os.environ:
    print("Using GITHUB_TOKEN from environment")
    GITHUB_TOKEN_HEADERS["Authorization"] = f"Bearer {os.environ['GITHUB_TOKEN']}"


def format_bzl(value, indent=0) -> str:
    """Format a Python value as a Starlark literal."""
    prefix = "    " * indent
    if isinstance(value, str):
        return repr(value)
    if isinstance(value, bool):
        return "True" if value else "False"
    if isinstance(value, int):
        return str(value)
    if value is None:
        return "None"
    if isinstance(value, list):
        if not value:
            return "[]"
        items = ",\n".join(f"{prefix}    {format_bzl(v, indent + 1)}" for v in value)
        return f"[\n{items},\n{prefix}]"
    if isinstance(value, dict):
        if not value:
            return "{}"
        items = ",\n".join(
            f"{prefix}    {format_bzl(k, indent + 1)}: {format_bzl(v, indent + 1)}"
            for k, v in value.items()
        )
        return f"{{\n{items},\n{prefix}}}"
    raise TypeError(f"Cannot format {type(value)} as Starlark")


def write_bzl(path: str, var_name: str, value):
    """Write a single variable assignment to a .bzl file."""
    with open(path, "w") as f:
        f.write(f"{var_name} = {format_bzl(value)}\n")


def read_bzl(path: Path, var_name: str):
    """Read a single variable assignment from a .bzl file."""
    text = path.read_text()
    # Extract the right-hand side of `VAR_NAME = ...`
    prefix = f"{var_name} = "
    assert text.startswith(prefix), f"Expected {path} to start with {prefix!r}"
    return ast.literal_eval(text[len(prefix) :])


def parse_version(ver: str) -> tuple:
    return tuple(map(int, ver.split(".")))


def version_less(a: str, b: str):
    return parse_version(a) < parse_version(b)


if TARGETS_PATH.exists():
    _targets_data = read_bzl(TARGETS_PATH, "TARGETS")
    target_map = {v: int(k[1:]) for k, v in _targets_data.items()}
else:
    target_map = {}


def compress_target(target: str) -> str:
    if target == "*":
        return "_"
    if target in target_map:
        return f"_{target_map[target]}"
    idx = len(target_map)
    target_map[target] = idx

    data = {f"_{v}": k for k, v in target_map.items()}
    write_bzl(str(TARGETS_PATH), "TARGETS", data)
    return f"_{idx}"


if RENAMES_PATH.exists():
    _renames_data = read_bzl(RENAMES_PATH, "ENTRIES")
    renames_map = {
        tuple(sorted(entry.items())): i for i, entry in enumerate(_renames_data)
    }
else:
    renames_map = {}


def compress_renames(renames: dict) -> int:
    entry = {k: v["to"] for k, v in sorted(renames.items())}
    key = tuple(sorted(entry.items()))

    if key in renames_map:
        return renames_map[key]
    idx = len(renames_map)
    renames_map[key] = idx

    entries = [None] * len(renames_map)
    for k, i in renames_map.items():
        entries[i] = dict(k)
    write_bzl(str(RENAMES_PATH), "ENTRIES", entries)
    return idx


if PROFILES_PATH.exists():
    _profiles_data = read_bzl(PROFILES_PATH, "ENTRIES")
    profiles_map = {
        tuple((k, tuple(v)) for k, v in sorted(entry.items())): i
        for i, entry in enumerate(_profiles_data)
    }
else:
    profiles_map = {}


def compress_profiles(profiles: dict) -> int:
    key = tuple((k, tuple(v)) for k, v in sorted(profiles.items()))

    if key in profiles_map:
        return profiles_map[key]
    idx = len(profiles_map)
    profiles_map[key] = idx

    entries = [None] * len(profiles_map)
    for k, i in profiles_map.items():
        entries[i] = {name: list(components) for name, components in k}
    write_bzl(str(PROFILES_PATH), "ENTRIES", entries)
    return idx


def fetch_url(url: str, params=None, headers={}, allow_not_found=False):
    i = 0
    while True:
        resp = None
        try:
            resp = requests.get(url, params=params, headers=headers)
            if resp.status_code == 404 and allow_not_found:
                return None
            resp.raise_for_status()
            return resp
        except requests.exceptions.RequestException as e:
            i += 1
            if (resp is not None and resp.status_code == 404) or i >= MAX_TRIES:
                raise
            print(e)
            time.sleep(RETRY_DELAY)


def translate_dump_manifest(channel: str, manifest: str, f):
    manifest = toml.loads(manifest)
    date = manifest["date"]
    rustc_version = manifest["pkg"]["rustc"]["version"].split()[0]
    renames_idx = compress_renames(manifest["renames"])
    strip_tail = "-preview"

    default_url_version = rustc_version if channel == "stable" else channel

    output = {}
    output["v"] = rustc_version
    output["d"] = date
    output["r"] = renames_idx
    if "profiles" in manifest:
        output["p"] = compress_profiles(manifest["profiles"])

    for pkg_name in sorted(manifest["pkg"].keys()):
        pkg = manifest["pkg"][pkg_name]
        pkg_name_stripped = (
            pkg_name[: -len(strip_tail)] if pkg_name.endswith(strip_tail) else pkg_name
        )
        pkg_targets = sorted(pkg["target"].keys())

        url_version = rustc_version
        url_target_map = {}
        for target_name in pkg_targets:
            target = pkg["target"][target_name]
            if not target["available"]:
                continue
            url = target["xz_url"]
            target_tail = "" if target_name == "*" else "-" + target_name
            start = f"{DIST_ROOT}/{date}/{pkg_name_stripped}-"
            end = f"{target_tail}.tar.xz"

            # Occurs in nightly-2019-01-10. Maybe broken or hirarerchy change?
            if url.startswith("nightly/"):
                url = DIST_ROOT + url[7:]

            # The target part may not be the same as current one.
            # This occurs in `pkg.rust-std.target.aarch64-apple-darwin` of nightly-2022-02-02,
            # which points to the URL of x86_64-apple-darwin rust-docs.
            if not url.endswith(end):
                assert url.startswith(
                    start + default_url_version + "-"
                ) and url.endswith(".tar.xz")
                url_target = url[
                    len(start + default_url_version + "-") : -len(".tar.xz")
                ]
                assert url_target in pkg_targets
                url_target_map[target_name] = url_target
                continue

            assert url.startswith(start) and url.endswith(end), f"Unexpected url: {url}"
            url_version = url[len(start) : -len(end)]

        pkg_data = {}
        if url_version != default_url_version:
            pkg_data["u"] = url_version
        for target_name in pkg_targets:
            # Forward to another URL.
            if target_name in url_target_map:
                url_target = url_target_map[target_name]
                assert pkg["target"][url_target] == pkg["target"][target_name]
                assert url_target != "*"
                url_target_id = int(compress_target(url_target)[1:])
                pkg_data[compress_target(target_name)] = url_target_id
                continue

            target = pkg["target"][target_name]
            if not target["available"]:
                continue
            url = target["xz_url"]
            # See above.
            if url.startswith("nightly/"):
                url = DIST_ROOT + url[7:]
            hash = target["xz_hash"]
            target_tail = "" if target_name == "*" else "-" + target_name
            expect_url = f"https://static.rust-lang.org/dist/{date}/{pkg_name_stripped}-{url_version}{target_tail}.tar.xz"
            assert url == expect_url, f"Unexpected url: {url}, expecting: {expect_url}"
            pkg_data[compress_target(target_name)] = hash

        output[pkg_name] = pkg_data

    f.write(f"MANIFEST = {format_bzl(output)}\n")


# Fetch and translate manifest file and return if it is successfully fetched.
def fetch_manifest(channel: str, version: str, out_path: Path) -> bool:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = out_path.with_suffix(".tmp")
    print(f"Fetching {channel} {version}")
    if channel == "stable":
        url = f"{DIST_ROOT}/channel-rust-{version}.toml"
    else:
        url = f"{DIST_ROOT}/{version}/channel-rust-{channel}.toml"
    manifest = fetch_url(url, allow_not_found=channel != "stable")
    if manifest is None:
        print("Not found, skipped")
        return False
    manifest = manifest.text
    MANIFEST_TMP_PATH.write_text(manifest)
    with open(tmp_path, "w") as fout:
        translate_dump_manifest(channel, manifest, fout)
    tmp_path.rename(out_path)
    return True


def manifest_ident(version):
    return f"manifest_{version}".replace(".", "_").replace("-", "_")


def write_index(path: str, versions_paths):
    out = ""
    for v, p in versions_paths:
        out += f"load('{p}', {manifest_ident(v)} = 'MANIFEST')\n"

    out += "INDEX = struct(**{\n"
    for v, _ in versions_paths:
        out += f"'{v}': {manifest_ident(v)},\n"

    latest_version, _ = versions_paths[-1]
    out += f"'latest': {manifest_ident(latest_version)},\n"
    out += "})\n"

    with open(path, "w") as f:
        f.write(out)


def update_stable_index(dir=Path("manifests/stable")):
    versions = sorted(
        (
            file.stem
            for file in dir.iterdir()
            if file.stem != "all" and file.suffix == ".bzl"
        ),
        key=parse_version,
    )

    versions = list(map(lambda version: tuple([version, f":{version}.bzl"]), versions))

    write_index(str(dir / "all.bzl"), versions)


def update_beta_index():
    update_nightly_index(dir=Path("manifests/beta"))


def update_nightly_index(dir=Path("manifests/nightly")):
    dates = sorted(file.stem for file in dir.rglob("*.bzl") if file.stem != "all")

    dates = list(
        map(
            lambda date: tuple(
                [date, f"{datetime.date.fromisoformat(date).year}/{date}.bzl"]
            ),
            dates,
        )
    )

    write_index(str(dir / "all.bzl"), dates)


def sync_stable_channel(*, stop_if_exists, max_update=None):
    GITHUB_TAGS_URL = "https://api.github.com/repos/rust-lang/rust/tags"
    PER_PAGE = 100

    versions = []
    page = 0
    while True:
        page += 1
        print(f"Fetching tags page {page}")
        resp = fetch_url(
            GITHUB_TAGS_URL,
            params={"per_page": PER_PAGE, "page": page},
            headers=GITHUB_TOKEN_HEADERS,
        ).json()
        versions.extend(
            tag["name"]
            for tag in resp
            if RE_STABLE_VERSION.match(tag["name"])
            and not version_less(tag["name"], MIN_STABLE_VERSION)
        )
        if len(resp) < PER_PAGE:
            break
    versions.sort(key=parse_version, reverse=True)

    print(f"Got {len(versions)} releases")

    processed = 0
    for version in versions:
        out_path = Path(f"manifests/stable/{version}.bzl")
        if out_path.exists():
            if not stop_if_exists:
                continue
            print(f"{version} is already fetched. Stopped")
            break
        assert fetch_manifest("stable", version, out_path), (
            f"Stable version {version} not found"
        )
        processed += 1
        assert max_update is None or processed <= max_update, "Too many versions"

    update_stable_index()


def sync_beta_channel(*, stop_if_exists, max_update=None):
    # Fetch the global nightly manifest to retrieve the latest nightly version.
    print("Fetching latest beta version")
    manifest = fetch_url(f"{DIST_ROOT}/channel-rust-beta.toml").text
    date = datetime.date.fromisoformat(toml.loads(manifest)["date"])
    print(f"The latest beta version is {date}")

    processed = 0
    date += datetime.timedelta(days=1)
    while date > MIN_BETA_DATE:
        date -= datetime.timedelta(days=1)
        date_str = date.isoformat()
        out_path = Path(f"manifests/beta/{date.year}/{date_str}.bzl")
        if out_path.exists():
            if not stop_if_exists:
                continue
            print(f"{date_str} is already fetched. Stopped")
            break
        if fetch_manifest("beta", date_str, out_path):
            processed += 1
        assert max_update is None or processed <= max_update, "Too many versions"
    update_beta_index()


def sync_nightly_channel(*, stop_if_exists, max_update=None):
    # Fetch the global nightly manifest to retrieve the latest nightly version.
    print("Fetching latest nightly version")
    manifest = fetch_url(f"{DIST_ROOT}/channel-rust-nightly.toml").text
    date = datetime.date.fromisoformat(toml.loads(manifest)["date"])
    print(f"The latest nightly version is {date}")

    processed = 0
    date += datetime.timedelta(days=1)
    while date > MIN_NIGHTLY_DATE:
        date -= datetime.timedelta(days=1)
        date_str = date.isoformat()
        out_path = Path(f"manifests/nightly/{date.year}/{date_str}.bzl")
        if out_path.exists():
            if not stop_if_exists:
                continue
            print(f"{date_str} is already fetched. Stopped")
            break
        if fetch_manifest("nightly", date_str, out_path):
            processed += 1
        assert max_update is None or processed <= max_update, "Too many versions"
    update_nightly_index()


def main():
    args = sys.argv[1:]
    if len(args) == 1 and args[0] in ["stable", "beta", "nightly"]:
        {
            "stable": sync_stable_channel,
            "beta": sync_beta_channel,
            "nightly": sync_nightly_channel,
        }[args[0]](stop_if_exists=True, max_update=SYNC_MAX_UPDATE)
    elif len(args) == 2 and args[0] == "stable":
        if args[1] == "all":
            sync_stable_channel(stop_if_exists=False)
        else:
            version = args[1]
            assert RE_STABLE_VERSION.match(version), "Invalid version"
            fetch_manifest("stable", version, Path(f"manifests/stable/{version}.bzl"))
            update_stable_index()
    elif len(args) == 2 and args[0] == "beta":
        if args[1] == "all":
            sync_beta_channel(stop_if_exists=False)
        else:
            date = datetime.date.fromisoformat(args[1])
            date_str = date.isoformat()
            fetch_manifest(
                "beta", date_str, Path(f"manifests/beta/{date.year}/{date_str}.bzl")
            )
            update_beta_index()
    elif len(args) == 2 and args[0] == "nightly":
        if args[1] == "all":
            sync_nightly_channel(stop_if_exists=False)
        else:
            date = datetime.date.fromisoformat(args[1])
            date_str = date.isoformat()
            fetch_manifest(
                "nightly",
                date_str,
                Path(f"manifests/nightly/{date.year}/{date_str}.bzl"),
            )
            update_nightly_index()
    else:
        print(
            """
Usage:
    {0} <channel>
        Auto-sync new versions from a channel.
    {0} <channel> <version>
        Force to fetch a specific version from a channel.
    {0} <channel> all
        Force to fetch all versions.
""".format(sys.argv[0])
        )
        exit(1)


if __name__ == "__main__":
    main()
