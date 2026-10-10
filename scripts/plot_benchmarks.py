"""Generate the public benchmark figure from measured JSON, using Matplotlib."""
import json
from pathlib import Path
from statistics import mean

import matplotlib

matplotlib.use('Agg')
import matplotlib.pyplot as plt


ROOT = Path(__file__).resolve().parent.parent
DATA = json.loads((ROOT / 'reports/benchmarks/v0.7.0-2026-10-10.json').read_text())


def values(prefix, field):
    return [r[field] for r in DATA['runs'] if r['label'].startswith(prefix)]


def main():
    plt.rcParams.update({
        'font.family': 'DejaVu Sans', 'font.size': 10,
        'text.color': '#172b35', 'axes.labelcolor': '#52616b',
        'xtick.color': '#52616b', 'ytick.color': '#172b35',
        'svg.fonttype': 'path', 'svg.hashsalt': 'minimalist-v070',
        'savefig.facecolor': '#ffffff',
    })
    fig, axes = plt.subplots(1, 3, figsize=(12, 4.6))
    fig.subplots_adjust(left=.13, right=.94, top=.71, bottom=.27, wspace=.8)
    fig.text(.035, .92, 'Less memory. Quiet in the tray.', fontsize=21, weight='bold')
    fig.text(.035, .855, 'Minimalist 0.7.0 vs. original control center  ·  lower is better', fontsize=11, color='#52616b')
    colors = ['#087f6b', '#a6b2bb', '#087f6b', '#a6b2bb']
    prefixes = ['native-visible', 'oem-visible', 'native-tray', 'oem-minimized']
    labels = ['Minimalist open', 'Original open', 'Minimalist tray', 'Original minimized']
    ram = [mean(values(p, 'median_working_set_bytes')) / 2**20 for p in prefixes]
    cpu = [mean(values(p, 'cpu_percent_whole_machine')) for p in prefixes]
    reply = [mean(values(p, 'window_ping_p95_ms')) for p in prefixes[:2]]
    panels = [
        ('RAM used', 'MiB · resident', ram, labels, colors, 230, [0, 100, 200], lambda v: f'{v:.1f}'),
        ('CPU usage', '% · whole PC', cpu, labels, colors, .62, [0, .2, .4, .6], lambda v: f'{v:.3f}%'),
        ('Idle window reply', 'ms · p95', reply, labels[:2], colors[:2], .48, [0, .2, .4], lambda v: f'{v:.3f}'),
    ]
    for ax, (title, unit, data, names, palette, xmax, ticks, fmt) in zip(axes, panels):
        ys = list(range(len(data)))
        ax.barh(ys, data, height=.5, color=palette, zorder=3)
        ax.set_yticks(ys, names, fontsize=8.5)
        ax.set_ylim(3.65, -.65)
        ax.set_xlim(0, xmax)
        ax.set_xticks(ticks)
        ax.set_xlabel(unit, fontsize=10, labelpad=9)
        ax.set_title(title, fontsize=12, weight='bold', loc='left', pad=17)
        ax.grid(axis='x', color='#e8edf0', linewidth=.8, zorder=0)
        ax.tick_params(length=0, labelsize=8.5, pad=5)
        for spine in ax.spines.values():
            spine.set_visible(False)
        for y, value in zip(ys, data):
            ax.text(value + xmax*.035, y, fmt(value), va='center', fontsize=9, weight='bold')
    fig.text(.035, .12, 'Open CPU: live dashboard vs. idle original page. Tray vs. minimized are different states.', fontsize=9, color='#52616b')
    fig.text(.035, .065, 'i7-8750H / GTX 1060  ·  two 30-second runs per state  ·  10 Oct 2026  ·  idle reply is not click latency', fontsize=9, color='#52616b')
    (ROOT / 'assets').mkdir(exist_ok=True)
    fig.savefig(ROOT / 'assets/benchmarks.svg', metadata={'Creator': 'scripts/plot_benchmarks.py', 'Date': DATA['date_utc']})
    svg = ROOT / 'assets/benchmarks.svg'
    svg.write_text('\n'.join(line.rstrip() for line in svg.read_text(encoding='utf-8').splitlines()) + '\n', encoding='utf-8')
    fig.savefig(ROOT / 'assets/benchmarks.png', dpi=160, metadata={'Software': 'scripts/plot_benchmarks.py'})
    plt.close(fig)
    print('Generated assets/benchmarks.svg and assets/benchmarks.png from measured data')


if __name__ == '__main__':
    main()
