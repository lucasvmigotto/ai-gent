# Containerized browsers for test automation

Shared by `qa:e2e`, `qa:strategy`, `frontend:build` (screenshots) and
`project:docs` (site e2e). Browsers never run on the host: no `chromedriver`,
`geckodriver`, `msedgedriver`, and no bare-metal `playwright install` of
browser binaries. Every browser an automated test drives comes from a
Selenium standalone container, pinned by digest per `shared/containers.md` §3.

## Images

One service per browser, from the `selenium/standalone-*` family
(verified live on Docker Hub; confirm the current version tag before
pinning — tags move with browser releases):

| Browser | Image | Notes |
|---|---|---|
| Chrome (default) | `selenium/standalone-chrome` | version tags like `153.0`; `-chromedriver-<v>` suffixed variants exist |
| Firefox | `selenium/standalone-firefox` | `-geckodriver-<v>` suffixed variants exist |
| Edge | `selenium/standalone-edge` | `-edgedriver-<v>` suffixed variants exist; only where Edge matters |
| Chromium | `selenium/standalone-chromium` | only when the project targets Chromium distinctly from Chrome |

## Compose mechanics

- Behind a `browser` compose profile, on the project's shared network, one
  stable service name per browser (`selenium-chrome`, `selenium-firefox`).
  Tests take the WebDriver endpoint from env
  (e.g. `SELENIUM_CHROME_URL=http://selenium-chrome:4444/wd/hub`), never a
  hardcoded host port.
- Healthcheck on the Grid status endpoint and gate tests on
  `service_healthy`, not on container start.
- Chrome needs shared memory: set `shm_size: 2g` (the Docker default
  `/dev/shm` is too small and crashes tabs under load). Size `cpus`,
  `mem_limit`, `pids_limit` per `shared/containers.md` §7 — browsers are
  heavy; one concurrent browser per ~2 CPUs is a starting point.
- The noVNC debug port (7900) stays off unless a human is watching a run.
- Standalone-per-browser is the default. A Grid hub plus
  `selenium/node-*` is only for a parallel matrix that needs one entry
  point; don't pay for the hub otherwise.
- CI runs the same images (`devsecops:pipeline` wires them in) — a green
  run locally means the same browsers as a green run in CI.

## Runner rule

- Greenfield: drive the standalone browsers with a Selenium WebDriver
  client in the project's stack language. Record the choice in `qa.md`.
- The project already uses Playwright and it's adequate: keep Playwright
  (`qa-quality.md`'s "use what the project already uses"), but its browsers
  still come from containers, not the host.
- Either way, capabilities pin the browser version to the image tag, and
  traces/screenshots/video-on-failure are uploaded as CI artifacts.

## WebKit gap (documented, not solved)

There is no `standalone-webkit` flavor. WebKit coverage runs only through
Playwright's container (`mcr.microsoft.com/playwright`, pinned by digest),
and only when the product's supported-browser list names WebKit/Safari.
Say so in `qa.md` and the README: the default matrix is Chrome + Firefox
(+ Edge where it matters); WebKit is a Playwright-container exception.
