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
    old_cpu = average('baseline-visible', 'cpu_percent_whole_machine')
    new_cpu = average('candidate-visible', 'cpu_percent_whole_machine')
    reduction = 100 * (1 - new_cpu / old_cpu)
    fig.text(.035, .93, f'{reduction:.0f}% less CPU with Cooling open', fontsize=21, weight='bold')
    fig.text(.035, .865, 'Minimalist 0.8.0 vs. 0.7.2  ·  lower is better', fontsize=11, color='#52616b')
    old, new, light = '#788995', '#087f6b', '#a6b2bb'
    ram_labels = ['Original open*', '0.7.2 open', '0.8.0 open', '0.8.0 tray']
    ram = [average('oem-visible', 'median_working_set_bytes', OEM) / 2**20]
    ram += [average(p, 'median_working_set_bytes') / 2**20
            for p in ['baseline-visible', 'candidate-visible', 'candidate-tray']]
    cpu = [old_cpu, new_cpu, average('candidate-tray', 'cpu_percent_whole_machine')]
    reply = [average(p, 'window_ping_p95_ms') for p in ['baseline-visible', 'candidate-visible']]
    panels = [
        ('RAM used', 'MiB · resident', ram, ram_labels, [light, old, new, new], 230, lambda v: f'{v:.1f}'),
        ('CPU usage', '% · whole PC', cpu, ['0.7.2 open', '0.8.0 open', '0.8.0 tray'], [old, new, new], max(cpu)*1.55, lambda v: f'{v:.3f}%'),
        ('Idle window reply', 'ms · p95', reply, ['0.7.2 open', '0.8.0 open'], [old, new], max(reply)*1.55, lambda v: f'{v:.3f}'),
    ]
    for ax, (title, unit, values, names, colors, xmax, fmt) in zip(axes, panels):
        ys = range(len(values))
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
    supply = DATA['power_source'].lower()
    fig.text(.035, .20, f'Native versions: Balanced / {supply}, i7-8750H / GTX 1060. Two 30s open runs; one 30s tray run each.', fontsize=9, color='#52616b')
    fig.text(.035, .14, '* Original RAM: earlier AC / Power Saver test. Includes native helper CPU and RAM; shared Windows hosts excluded.', fontsize=9, color='#52616b')
    fig.text(.035, .08, 'Idle reply measures window messages, not clicks. Zero CPU means below counter resolution. Full method and raw data linked below.', fontsize=9, color='#52616b')
    fig.savefig(ROOT / 'assets/benchmarks.svg', metadata={'Creator': 'scripts/plot_benchmarks.py', 'Date': DATA['date_utc']})
    svg = ROOT / 'assets/benchmarks.svg'
    svg.write_text('\n'.join(line.rstrip() for line in svg.read_text(encoding='utf-8').splitlines()) + '\n', encoding='utf-8')
    fig.savefig(ROOT / 'assets/benchmarks.png', dpi=160, metadata={'Software': 'scripts/plot_benchmarks.py'})
    plt.close(fig)
    print('Generated benchmark SVG and PNG from measured data')


if __name__ == '__main__':
    main()
