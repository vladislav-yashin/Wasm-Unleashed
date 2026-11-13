#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build server-delivered landing HTML and Tailwind CSS without compiling the game."""
import argparse
from html import escape
import os
from pathlib import Path
import shutil
import subprocess
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[2]
WEB = ROOT / 'packaging/web'


def build(output: Path, site_url: str | None = None):
    """Emit complete HTML; no JavaScript is needed to render or index the page."""
    base = None
    if site_url:
        parsed = urlsplit(site_url)
        if (parsed.scheme not in ('https', 'http') or not parsed.hostname
                or parsed.query or parsed.fragment or parsed.username or parsed.password):
            raise ValueError('Site URL must be an absolute HTTP(S) URL without credentials, query or fragment')
        base = site_url.rstrip('/') + '/'
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run(['npm', 'ci', '--no-audit', '--no-fund'], cwd=WEB, check=True)
    subprocess.run([str(WEB / 'node_modules/.bin/tailwindcss'), '-i', 'landing.css',
                    '-o', str(output.resolve() / 'landing.css'), '--minify'], cwd=WEB, check=True)
    page = (WEB / 'index.html').read_text()
    if base:
        url = escape(base, quote=True)
        page = page.replace('</head>', f'<link rel="canonical" href="{url}">\n'
                            f'<meta property="og:url" content="{url}">\n'
                            f'<meta property="og:image" content="{url}porsche-993-alpine.webp">\n'
                            f'<meta name="twitter:image" content="{url}porsche-993-alpine.webp">\n</head>')
        (output / 'sitemap.xml').write_text(
            '<?xml version="1.0" encoding="UTF-8"?>\n'
            '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">'
            f'<url><loc>{url}</loc></url></urlset>\n')
    else:
        (output / 'sitemap.xml').unlink(missing_ok=True)
    (output / 'index.html').write_text(page)
    robots = 'User-agent: *\nAllow: /\nDisallow: /data/\n'
    if base:
        robots += f'Sitemap: {base}sitemap.xml\n'
    (output / 'robots.txt').write_text(robots)
    for name in ('porsche-993-alpine.webp', 'favicon.svg', 'game.html'):
        shutil.copyfile(WEB / name, output / name)
    shutil.copyfile(ROOT / 'LICENSE', output / 'LICENSE')
    tailwind_license = (WEB / 'node_modules/tailwindcss/LICENSE').read_text()
    (output / 'THIRD-PARTY-NOTICES.txt').write_text(
        'Tailwind CSS — MIT license\nhttps://tailwindcss.com/\n\n' + tailwind_license)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'dist')
    parser.add_argument('--site-url', default=os.environ.get('SITE_URL'))
    args = parser.parse_args()
    build(args.output, args.site_url)
