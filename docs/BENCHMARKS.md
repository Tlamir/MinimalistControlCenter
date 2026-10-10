# Benchmarks

![RAM, CPU and idle window-response comparison](../assets/benchmarks.svg)

| App / state | RAM, resident | Private commit | CPU, whole PC | Idle window reply, p95 |
|---|---:|---:|---:|---:|
| Minimalist 0.7.0 · open | 16.2 MiB | 2.6 MiB | 0.411–0.436% | 0.251–0.262 ms |
| Original · open | 175.8 MiB | 127.1 MiB | 0.004–0.017% | 0.329–0.344 ms |
| Minimalist 0.7.0 · tray | 16.2 MiB | 2.5 MiB | 0–0.004% | — |
| Original · minimized | 171.8 MiB | 124.2 MiB | 0.004% | — |

Visible RAM was about **91% lower**. The live Minimalist dashboard used more CPU than the original's idle default page. In the tray, Minimalist stops OS and firmware polling and launched no helpers in these runs. Versions 0.7.1 and 0.7.2 retain this polling policy; the charts measure **0.7.0**, not a fresh benchmark of later releases.

## Method

Measured on 10 October 2026: Monster Tulpar T5 V17.1 / BIOS N.1.06, i7-8750H, GTX 1060, 16 GB RAM, 12 logical processors; Windows 25H2 build 26200.9457. AC power, Power Saver, normal desktop activity. Only one controller ran at a time. Each state had two 30-second runs after settling.

- **RAM:** average of the two runs' median aggregate working sets, sampled once a second. Shared pages may be counted twice and brief helpers can be missed. Minimalist's visible maximum sampled RAM was 34.3 MiB.
- **CPU:** total time divided by elapsed time and 12 logical processors. Native child helpers, including exited ones, are included through Windows job accounting. The original includes its GUI, tray and launcher service; shared Windows/WMI hosts are excluded. A zero means below counter resolution.
- **Response:** p95 idle `WM_NULL` reply time, averaged across runs in the chart. No timeouts. This measures message dispatch, not button actions or the owner's intermittent hiccups.

The pages have different features: Minimalist shows live counters and checks firmware mode; the original uses its default page. Original minimized and Minimalist tray are different states. These results don't establish FPS, battery, cooling or general responsiveness improvements.

[JSON report](../reports/benchmarks/v0.7.0-2026-10-10.json) includes per-run values, executable hashes and process scope. [Measurement script](../scripts/measure-app.ps1) · [Chart script](../scripts/plot_benchmarks.py).

## Repeat

Run the selected app, settle it, then measure its known process IDs. Use elevated PowerShell if the app is elevated:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-app.ps1 -ProcessIds <GUI_PID> -Label native-visible -Seconds 30 -Output reports/local/native-visible.json
```

In PowerShell, pass multiple roots with `-ProcessIds @(GUI_PID,TRAY_PID)` and an existing service with `-ExternalProcessIds @(SERVICE_PID)`. Optional `-WindowHandle` measures idle replies. Repeat twice per state; keep power and AC unchanged. The script changes no hardware settings and leaves the app running. Service children created later and shared Windows hosts are not automatically attributed.
