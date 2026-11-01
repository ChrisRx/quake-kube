"""Writes a content image's profile: the files the browser client reads on each of a preset's
maps, which the play page fetches before starting the engine.

For each map, starts a server from the preset in hack/k3d/presets.yaml in the dev cluster,
joins it in headless Chromium with the engine logging the files it opens, and records them.
Files read on at least half the maps are common to all of them.

Usage: hack/profile.sh <preset> <output> [presets file], e.g.
  hack/profile.sh cpma content/cpma/profile.json
"""
import collections
import json
import re
import subprocess
import sys
import time
from pathlib import Path

import yaml
from playwright.sync_api import sync_playwright

WEB = "https://quake.172-28-0-100.sslip.io"
NAMESPACE = "quake-kube"
OPENED = re.compile(r"FS_FOpenFileRead: (.+?) \(found in '\./(.+?\.pk3)'\)")


def kubectl(*args, input=None):
    return subprocess.run(
        ["kubectl", "-n", NAMESPACE, *args], input=input, text=True, check=True, capture_output=True
    ).stdout


def server_name(m):
    return "profile-" + re.sub(r"[^a-z0-9]+", "-", m.lower()).strip("-")


def main():
    preset_name, output = sys.argv[1], Path(sys.argv[2])
    presets_file = Path(sys.argv[3] if len(sys.argv) > 3 else "hack/k3d/presets.yaml")
    presets = yaml.safe_load(presets_file.read_text())["gateway"]["presets"]
    preset = next(p for p in presets if p["name"] == preset_name)
    maps = [m.lower() for m in preset["maps"]]

    for m in maps:
        kubectl("apply", "-f", "-", input=json.dumps({
            "apiVersion": "quake.kube/v1",
            "kind": "QuakeServer",
            "metadata": {"name": server_name(m), "namespace": NAMESPACE},
            "spec": {**preset.get("spec", {}), "maps": [{"name": m}]},
        }))

    reads = {}
    try:
        with sync_playwright() as p:
            browser = p.chromium.launch(
                channel="chromium",
                headless=True,
                args=["--use-angle=vulkan", "--enable-features=Vulkan"],
            )
            for m in maps:
                reads[m] = read_files(browser, m)
            browser.close()
    finally:
        for m in maps:
            kubectl("delete", "quakeserver", server_name(m), "--wait=false")

    # Files read on at least half the maps are fetched for every map.
    counts = collections.Counter(f for files in reads.values() for f in files)
    common = {f for f, n in counts.items() if 2 * n >= len(maps)}
    profile = {
        "common": by_pak(common),
        "maps": {m: by_pak(files - common) for m, files in sorted(reads.items())},
    }
    output.write_text(json.dumps(profile, indent=1, sort_keys=True) + "\n")
    print(f"wrote {output}: {len(common)} common files, "
          + ", ".join(f"{m} {len(files - common)}" for m, files in sorted(reads.items())))


def read_files(browser, m):
    """Joins the map's server and returns the (pak, file) pairs the engine opened."""
    name = server_name(m)
    kubectl("wait", f"quakeserver/{name}", "--for=jsonpath={.status.ready}=true", "--timeout=600s")
    page = browser.new_page(ignore_https_errors=True)
    lines = []
    page.on("console", lambda msg: lines.append(msg.text))

    # The engine logs every file it opens with fs_debug.
    def add_fs_debug(route):
        response = route.fetch()
        body = response.text().replace(
            '"+set", "cl_motd", "0",', '"+set", "cl_motd", "0", "+set", "fs_debug", "1",'
        )
        route.fulfill(response=response, body=body)

    page.route("**/play/*/*/", add_fs_debug)
    url = f"{WEB}/play/{NAMESPACE}/{name}"
    # Until the server answers queries, the page can't join.
    deadline = time.time() + 300
    while not page.request.get(f"{url}/info").ok:
        if time.time() > deadline:
            sys.exit(f"{m}'s server doesn't answer")
        time.sleep(2)
    page.goto(url + "/")
    page.fill("#player-name", "profile")
    if page.is_visible("#eula"):
        page.check("#eula")
    page.click("#join button")
    deadline = time.time() + 600
    while not any("CL_InitCGame:" in line for line in lines):
        if time.time() > deadline:
            sys.exit(f"{m} didn't load: {page.text_content('#status')}")
        page.wait_for_timeout(1000)
    # What's loaded right after cgame, e.g. the map's sounds.
    page.wait_for_timeout(5000)

    opened = set()
    for line in lines:
        match = OPENED.search(line)
        if match:
            f, pak = match.groups()
            opened.add((pak, f))
    if not opened:
        sys.exit(f"{m}: no files logged, did the play page's arguments change?")
    # The engine finds files whatever their case, so names are taken from the paks.
    names = {}
    files = set()
    for pak, f in opened:
        if pak not in names:
            index = page.request.get(f"{url}/paks/{pak}.json").json()
            names[pak] = {n.lower(): n for n in index["files"]}
        files.add((pak, names[pak].get(f.lower(), f)))
    page.close()
    print(f"{m}: {len(files)} files", flush=True)
    return files


def by_pak(files):
    paks = collections.defaultdict(list)
    for pak, f in sorted(files):
        paks[pak].append(f)
    return dict(paks)


if __name__ == "__main__":
    main()
