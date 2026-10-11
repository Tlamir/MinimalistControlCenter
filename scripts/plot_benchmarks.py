"""Generate the main benchmark chart from published measurements."""
import json
from pathlib import Path
from statistics import mean

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent.parent
DATA = json.loads((ROOT / 'reports/benchmarks/v0.8.0-2026-10-11.json').read_text())
OEM = json.loads((ROOT / 'reports/benchmarks/v0.7.0-2026-10-10.json').read_text())


def average(prefix, field, dataset=DATA):
    return mean(r[field] for r in dataset['runs'] if r['label'].startswith(prefix))


def main():
    plt.rcParams.update({
        'font.family': 'DejaVu Sans', 'font.size': 10, 'text.color': '#172b35',
        'axes.labelcolor': '#52616b', 'xtick.color': '#52616b', 'ytick.color': '#172b35',
        'svg.fonttype': 'path', 'svg.hashsalt': 'minimalist-v080', 'savefig.facecolor': '#ffffff',
    })
    fig, axes = plt.subplots(1, 3, figsize=(12, 5.2))
    fig.subplots_adjust(left=.13, right=.94, top=.72, bottom=.32, wspace=.8)
    fig.text(.035, .93, 'Original control center vs. Minimalist', fontsize=21, weight='bold')
    fig.text(.035, .865, 'Measured on the same laptop in separate sessions; conditions differ', fontsize=11, color='#52616b')
    old, new = '#788995', '#087f6b'
    names = ['Original open', 'Minimalist open', 'Original minimized', 'Minimalist tray']
    colors = [old, new, old, new]
    ram = [average('oem-visible', 'median_working_set_bytes', OEM) / 2**20,
           average('candidate-visible', 'median_working_set_bytes') / 2**20,
           average('oem-minimized', 'median_working_set_bytes', OEM) / 2**20,
           average('candidate-tray', 'median_working_set_bytes') / 2**20]
    cpu = [average('oem-visible', 'cpu_percent_whole_machine', OEM),
           average('candidate-visible', 'cpu_percent_whole_machine'),
           average('oem-minimized', 'cpu_percent_whole_machine', OEM),
           average('candidate-tray', 'cpu_percent_whole_machine')]
    reply = [average('oem-visible', 'window_ping_p95_ms', OEM),
             average('candidate-visible', 'window_ping_p95_ms')]
    panels = [
        ('RAM used', 'MiB - resident', ram, names, colors, max(ram)*1.5, lambda v: f'{v:.1f}'),
        ('CPU usage', '% - whole PC (0-100)', cpu, names, colors, 100, lambda v: f'{v:.3f}%' if v else '0% observed*'),
        ('Idle window reply', 'ms - p95 (open)', reply, names[:2], [old, new], max(reply)*1.55, lambda v: f'{v:.3f}'),
    ]
    for ax, (title, unit, values, names, colors, xmax, fmt) in zip(axes, panels):
        ys = range(len(values))
        if title == 'CPU usage':
            ax.barh(ys, [100] * len(values), height=.5, color='#edf1f3', zorder=1)
            ax.set_xticks([0, 50, 100])
        ax.barh(ys, values, height=.5, color=colors, zorder=3)
        ax.set_yticks(ys, names, fontsize=8.5)
        ax.set_ylim(3.65, -.65)
        ax.set_xlim(0, xmax)
        ax.set_xlabel(unit, fontsize=10, labelpad=9)
        ax.set_title(title, fontsize=12, weight='bold', loc='left', pad=17)
        ax.grid(axis='x', color='#e8edf0', linewidth=.8, zorder=0)
        ax.tick_params(length=0, labelsize=8.5, pad=5)
        for spine in ax.spines.values():
            spine.set_visible(False)
        for y, value in zip(ys, values):
            ax.text(value + xmax*.035, y, fmt(value), va='center', fontsize=9, weight='bold')
    fig.text(.035, .20, 'Original: AC / Power Saver, default page. Minimalist 0.8.0: battery / Balanced, connected Cooling page.', fontsize=9, color='#52616b')
    fig.text(.035, .14, 'Both apps averaged less than 0.1% CPU open. * Zero means below counter resolution during the 30s tray test.', fontsize=9, color='#52616b')
    fig.text(.035, .08, 'Includes app helpers. Conditions differ; battery savings unmeasured. Idle replies are window messages, not clicks.', fontsize=9, color='#52616b')
    fig.savefig(ROOT / 'assets/benchmarks-idle-2026-10-11.svg', metadata={'Creator': 'scripts/plot_benchmarks.py', 'Date': DATA['date_utc']})
    svg = ROOT / 'assets/benchmarks-idle-2026-10-11.svg'
    svg.write_text('\n'.join(line.rstrip() for line in svg.read_text(encoding='utf-8').splitlines()) + '\n', encoding='utf-8')
    fig.savefig(ROOT / 'assets/benchmarks.png', dpi=160, metadata={'Software': 'scripts/plot_benchmarks.py'})
    plt.close(fig)
    print('Generated benchmark SVG and PNG from measured data')


if __name__ == '__main__':
    main()
