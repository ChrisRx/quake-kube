"""Captures the web app's screenshots in the book from a served browser preview, driving it in
headless Chromium the way a person would, and playing a game for the pause menu.

Usage: hack/screenshots.sh, which serves the preview and shrinks the images, runs
  hack/screenshots.py <preview URL> <output directory>
"""
import sys
from pathlib import Path

from playwright.sync_api import sync_playwright

# A third server would use up the preview's quota, which turns it red and dims Create server.
SERVERS = [
    {"name": "Friday Night", "game": "OpenArena", "bots": 5, "maps": 3},
    {"name": "Capture Practice", "game": "OpenArena CTF", "bots": 7, "maps": 2},
]
# Every server has finished appearing to start.
READY = """count => {
  const servers = [...document.querySelectorAll(".server")];
  return servers.length === count && servers.every((server) =>
    !/scheduling|pulling|creating|starting/i.test(server.textContent)
    && server.querySelector(".play").href);
}"""
# From the top of the page to the end of the list, with a margin.
LIST_CLIP = """() => {
  const list = document.getElementById("servers").getBoundingClientRect();
  return { x: list.x - 16, y: 0, width: list.width + 32, height: list.bottom + 16 };
}"""


def fill_create_dialog(page, server):
    page.click("#create-open")
    page.wait_for_selector("#create[open]")
    page.fill('#create-form [name="name"]', server["name"])
    page.select_option("#presets", label=server["game"])
    page.fill('#create-form [name="bots"]', str(server["bots"]))
    page.wait_for_selector(".mp-tile")
    # One map starts out picked. The rest are spaced out so the picks' order shows.
    unpicked = page.locator(".mp-tile[aria-pressed=false]")
    for i in range(server["maps"] - 1):
        unpicked.nth(i * 2).click()


def submit_create_dialog(page):
    page.click("#create-submit")
    page.wait_for_selector("#create:not([open])", state="attached")


def main():
    url, out = sys.argv[1], Path(sys.argv[2])
    with sync_playwright() as p:
        # The game gets no WebGL from headless Chromium's software rendering.
        browser = p.chromium.launch(
            channel="chromium",
            headless=True,
            args=["--use-angle=vulkan", "--enable-features=Vulkan"],
        )
        context = browser.new_context(viewport={"width": 1100, "height": 900}, device_scale_factor=2)
        page = context.new_page()
        page.set_default_timeout(15000)
        page.goto(url)
        page.wait_for_selector("#create-open:not([hidden])")

        # Saving a list makes the dialog show the lists menu too.
        fill_create_dialog(page, SERVERS[0])
        page.get_by_role("button", name="Save as list").click()
        page.fill(".mp-save input", "Favorites")
        page.locator(".mp-save").get_by_role("button", name="Save").click()
        page.wait_for_timeout(1500)
        page.locator("#create").screenshot(path=out / "create-server.png")
        submit_create_dialog(page)
        for server in SERVERS[1:]:
            fill_create_dialog(page, server)
            submit_create_dialog(page)

        page.wait_for_function(READY, arg=len(SERVERS), timeout=30000)
        page.wait_for_timeout(1500)
        # The preview's notice isn't part of the web app.
        page.locator("main > p.hint:not(#hint)").evaluate("notice => notice.remove()")
        page.locator(".server .summary").first.click()
        page.wait_for_timeout(1500)
        page.screenshot(path=out / "server-list.png", clip=page.evaluate(LIST_CLIP))

        page.locator(".server .play").first.click()
        page.wait_for_selector("#join form")
        page.wait_for_timeout(2000)
        page.fill("#player-name", "Visitor")
        page.evaluate("document.activeElement.blur()")
        page.wait_for_timeout(300)
        page.locator("#join form").screenshot(path=out / "join-server.png")

        page.set_viewport_size({"width": 1100, "height": 620})
        page.click('#join button[type="submit"]')
        page.wait_for_selector("#pause:not([hidden])", timeout=60000)
        page.wait_for_timeout(1000)
        page.click("#resume")
        page.wait_for_function("document.pointerLockElement !== null")
        # Long enough to be in the map, with the bots moving around.
        page.wait_for_timeout(10000)
        page.evaluate("document.exitPointerLock()")
        page.wait_for_selector("#pause:not([hidden])")
        page.wait_for_timeout(1500)
        page.screenshot(path=out / "pause-menu.png")
        browser.close()


if __name__ == "__main__":
    main()
