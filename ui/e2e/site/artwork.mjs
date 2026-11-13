// Artwork for the fictional demo catalog: posters, backdrops and portraits drawn as HTML and
// SVG and rendered to PNG by Chromium. Each title has two colours, an accent and a motif
// (`demo-data.mjs`); the same id always gives the same picture.

/** A small deterministic random generator (mulberry32) per title. */
function random(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function stars(rnd, w, h, count, maxY) {
  return Array.from({ length: count }, () => {
    const r = (rnd() * 1.4 + 0.4).toFixed(2);
    const o = (rnd() * 0.6 + 0.3).toFixed(2);
    return `<circle cx="${(rnd() * w).toFixed(1)}" cy="${(rnd() * h * maxY).toFixed(1)}" r="${r}" fill="#fff" opacity="${o}"/>`;
  }).join('');
}

/** Layered ridges from `y` downwards, each a little darker. */
function ridges(rnd, w, h, y, color, layers, sharp) {
  let out = '';
  for (let l = 0; l < layers; l += 1) {
    const base = h * (y + l * 0.07);
    const points = [];
    const steps = sharp ? 7 : 14;
    for (let i = 0; i <= steps; i += 1) {
      const amp = h * (sharp ? 0.12 : 0.035) * (1 - l * 0.2);
      points.push(`${((i / steps) * w).toFixed(1)},${(base - rnd() * amp).toFixed(1)}`);
    }
    const shape = sharp ? `L${points.join(' L')}` : `L${points.join(' L')}`;
    out += `<path d="M0,${h} ${shape} L${w},${h} Z" fill="${color}" opacity="${0.35 + l * 0.2}"/>`;
  }
  return out;
}

function waves(w, h, y, color, count) {
  let out = '';
  for (let i = 0; i < count; i += 1) {
    const base = h * (y + i * 0.06);
    const amp = h * 0.02;
    let d = `M0,${base}`;
    for (let x = 0; x <= w; x += w / 8) {
      d += ` Q${x + w / 32},${base - amp} ${x + w / 16},${base} T${x + w / 8},${base}`;
    }
    out += `<path d="${d} L${w},${h} L0,${h} Z" fill="${color}" opacity="${0.18 + i * 0.12}"/>`;
  }
  return out;
}

/** The motif of a title as SVG elements in a `w` by `h` box. */
function motif(t, w, h) {
  const { accent, motif: kind } = t.look;
  const rnd = random(t.id);
  const m = Math.min(w, h);
  switch (kind) {
    case 'aurora': {
      let bands = '';
      for (let i = 0; i < 4; i += 1) {
        const y = h * (0.18 + i * 0.07);
        bands += `<path d="M${-w * 0.1},${y + h * 0.1} C${w * 0.3},${y - h * 0.12} ${w * 0.6},${y + h * 0.16} ${w * 1.1},${y - h * 0.04}" stroke="${accent}" stroke-width="${m * (0.05 - i * 0.008)}" fill="none" opacity="${0.5 - i * 0.09}" filter="url(#blur)"/>`;
      }
      return stars(rnd, w, h, 80, 0.6) + bands + ridges(rnd, w, h, 0.72, '#020611', 3, true);
    }
    case 'waves':
      return `<circle cx="${w * 0.7}" cy="${h * 0.3}" r="${m * 0.12}" fill="${accent}" opacity="0.85"/>${waves(w, h, 0.55, accent, 5)}`;
    case 'grid': {
      let lines = '';
      const vx = w / 2;
      const vy = h * 0.42;
      for (let i = -8; i <= 8; i += 1) {
        lines += `<line x1="${vx}" y1="${vy}" x2="${vx + i * w * 0.16}" y2="${h}" stroke="${accent}" stroke-width="1.2" opacity="0.55"/>`;
      }
      for (let i = 1; i <= 9; i += 1) {
        const y = vy + (h - vy) * (i / 9) ** 1.8;
        lines += `<line x1="0" y1="${y}" x2="${w}" y2="${y}" stroke="${accent}" stroke-width="1.2" opacity="0.5"/>`;
      }
      return `${lines}<rect x="${w * 0.36}" y="${h * 0.16}" width="${w * 0.28}" height="${h * 0.26}" fill="none" stroke="${accent}" stroke-width="${m * 0.012}" opacity="0.9"/>`;
    }
    case 'planet':
      return `${stars(rnd, w, h, 120, 1)}<circle cx="${w * 0.5}" cy="${h * 0.38}" r="${m * 0.26}" fill="url(#planet)"/><ellipse cx="${w * 0.5}" cy="${h * 0.38}" rx="${m * 0.44}" ry="${m * 0.07}" fill="none" stroke="${accent}" stroke-width="${m * 0.012}" opacity="0.8" transform="rotate(-14 ${w * 0.5} ${h * 0.38})"/>`;
    case 'gear': {
      const cx = w * 0.5;
      const cy = h * 0.36;
      const r = m * 0.3;
      let ticks = '';
      for (let i = 0; i < 60; i += 1) {
        const a = (i / 60) * Math.PI * 2;
        const inner = r * (i % 5 === 0 ? 0.84 : 0.92);
        ticks += `<line x1="${cx + Math.cos(a) * inner}" y1="${cy + Math.sin(a) * inner}" x2="${cx + Math.cos(a) * r}" y2="${cy + Math.sin(a) * r}" stroke="${accent}" stroke-width="${i % 5 === 0 ? 3 : 1.2}"/>`;
      }
      return `<circle cx="${cx}" cy="${cy}" r="${r * 1.08}" fill="none" stroke="${accent}" stroke-width="2" opacity="0.6"/>${ticks}<line x1="${cx}" y1="${cy}" x2="${cx + r * 0.5}" y2="${cy - r * 0.45}" stroke="${accent}" stroke-width="4" stroke-linecap="round"/><line x1="${cx}" y1="${cy}" x2="${cx - r * 0.15}" y2="${cy + r * 0.7}" stroke="${accent}" stroke-width="2.5" stroke-linecap="round"/><circle cx="${cx}" cy="${cy}" r="6" fill="${accent}"/>`;
    }
    case 'dunes':
      return `<circle cx="${w * 0.3}" cy="${h * 0.3}" r="${m * 0.14}" fill="${accent}" opacity="0.75"/>${ridges(rnd, w, h, 0.55, accent, 4, false)}`;
    case 'rails': {
      const vx = w * 0.5;
      const vy = h * 0.4;
      let out = `<line x1="${vx}" y1="${vy}" x2="${w * 0.12}" y2="${h}" stroke="${accent}" stroke-width="3"/><line x1="${vx}" y1="${vy}" x2="${w * 0.88}" y2="${h}" stroke="${accent}" stroke-width="3"/>`;
      for (let i = 1; i <= 10; i += 1) {
        const f = (i / 10) ** 1.7;
        const y = vy + (h - vy) * f;
        const half = w * 0.38 * f + 2;
        out += `<line x1="${vx - half}" y1="${y}" x2="${vx + half}" y2="${y}" stroke="${accent}" stroke-width="${1 + f * 4}" opacity="0.6"/>`;
      }
      return `${stars(rnd, w, h, 40, 0.35)}<circle cx="${vx}" cy="${vy}" r="${m * 0.03}" fill="${accent}"/><circle cx="${vx}" cy="${vy}" r="${m * 0.1}" fill="${accent}" opacity="0.15"/>${out}`;
    }
    case 'sun':
      return `<circle cx="${w * 0.5}" cy="${h * 0.5}" r="${m * 0.26}" fill="${accent}" opacity="0.9"/>${ridges(rnd, w, h, 0.58, '#000', 3, false)}`;
    case 'mountains':
      return `${stars(rnd, w, h, 30, 0.3)}<circle cx="${w * 0.72}" cy="${h * 0.2}" r="${m * 0.06}" fill="${accent}" opacity="0.9"/>${ridges(rnd, w, h, 0.5, '#000', 4, true)}`;
    case 'lighthouse': {
      const x = w * 0.62;
      const top = h * 0.3;
      return `${stars(rnd, w, h, 50, 0.5)}<path d="M${x},${top} L${-w * 0.2},${top - h * 0.12} L${-w * 0.2},${top + h * 0.1} Z" fill="${accent}" opacity="0.22"/><path d="M${x - m * 0.04},${top} L${x + m * 0.04},${top} L${x + m * 0.07},${h * 0.66} L${x - m * 0.07},${h * 0.66} Z" fill="#e9edf2"/><rect x="${x - m * 0.05}" y="${top - m * 0.06}" width="${m * 0.1}" height="${m * 0.06}" fill="${accent}"/>${waves(w, h, 0.64, '#9fb6c8', 4)}`;
    }
    case 'signal': {
      let arcs = '';
      for (let i = 1; i <= 6; i += 1) {
        arcs += `<circle cx="${w * 0.5}" cy="${h * 0.42}" r="${m * 0.07 * i}" fill="none" stroke="${accent}" stroke-width="${m * 0.008}" opacity="${0.9 - i * 0.13}"/>`;
      }
      return `${arcs}<line x1="${w * 0.5}" y1="${h * 0.42}" x2="${w * 0.5}" y2="${h}" stroke="${accent}" stroke-width="3"/>${ridges(rnd, w, h, 0.78, '#000', 2, false)}`;
    }
    default:
      return '';
  }
}

function svg(t, w, h) {
  const { accent } = t.look;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" style="position:absolute;inset:0">
<defs><filter id="blur"><feGaussianBlur stdDeviation="${Math.min(w, h) * 0.02}"/></filter>
<radialGradient id="planet" cx="35%" cy="30%"><stop offset="0" stop-color="${accent}"/><stop offset="1" stop-color="${t.look.to}"/></radialGradient></defs>
${motif(t, w, h)}</svg>`;
}

const page = (w, h, body) => `<!doctype html><html><head><meta charset="utf-8"><style>
*{margin:0;padding:0;box-sizing:border-box}
html,body{width:${w}px;height:${h}px;overflow:hidden}
.frame{position:relative;width:${w}px;height:${h}px;overflow:hidden}
</style></head><body>${body}</body></html>`;

function posterHtml(t, credit) {
  const w = 500;
  const h = 750;
  const size = t.title.length > 16 ? 40 : t.title.length > 10 ? 50 : 62;
  return page(
    w,
    h,
    `<div class="frame" style="background:linear-gradient(170deg,${t.look.from},${t.look.to})">
${svg(t, w, h)}
<div style="position:absolute;inset:auto 0 0 0;height:46%;background:linear-gradient(transparent,rgba(0,0,0,.72))"></div>
<div style="position:absolute;left:34px;right:34px;bottom:40px;color:#f7f4ee;font-family:'Segoe UI',Arial,sans-serif">
<div style="font-size:15px;letter-spacing:.32em;text-transform:uppercase;opacity:.82;margin-bottom:12px">${credit}</div>
<div style="font-family:Georgia,'Times New Roman',serif;font-size:${size}px;line-height:1.02;font-weight:700;letter-spacing:.01em">${t.title}</div>
<div style="margin-top:16px;height:3px;width:64px;background:${t.look.accent}"></div>
<div style="margin-top:12px;font-size:14px;letter-spacing:.28em;opacity:.75">${t.kind === 'movie' ? 'IM KINO' : 'DIE SERIE'} · ${t.year}</div>
</div></div>`,
  );
}

function backdropHtml(t) {
  const w = 1280;
  const h = 720;
  return page(
    w,
    h,
    `<div class="frame" style="background:linear-gradient(160deg,${t.look.from},${t.look.to})">${svg(t, w, h)}</div>`,
  );
}

function portraitHtml(p) {
  const w = 300;
  const h = 450;
  const bg = `hsl(${p.hue} 30% 26%)`;
  const bg2 = `hsl(${(p.hue + 30) % 360} 35% 42%)`;
  const fg = `hsl(${p.hue} 25% 70%)`;
  return page(
    w,
    h,
    `<div class="frame" style="background:linear-gradient(160deg,${bg},${bg2})">
<svg width="${w}" height="${h}" style="position:absolute;inset:0"><circle cx="150" cy="170" r="72" fill="${fg}"/>
<path d="M30,450 C30,330 90,270 150,270 C210,270 270,330 270,450 Z" fill="${fg}"/></svg></div>`,
  );
}

async function render(browser, w, h, html) {
  const tab = await browser.newPage({ viewport: { width: w, height: h } });
  await tab.setContent(html);
  const png = await tab.screenshot({ type: 'png' });
  await tab.close();
  return png;
}

/** PNG bytes of a title's poster; `credit` is the line above the title. */
export const poster = (browser, t, credit) => render(browser, 500, 750, posterHtml(t, credit));
export const backdrop = (browser, t) => render(browser, 1280, 720, backdropHtml(t));
export const portrait = (browser, p) => render(browser, 300, 450, portraitHtml(p));
