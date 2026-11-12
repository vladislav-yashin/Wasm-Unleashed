// Optional browser-check instrumentation for active WebGL contexts and frame scheduling.
const profile = window.nfsWebglProfile = { calls: {}, slow: [], frames: 0, gaps: [], maxGap: 0 };
const methods = new Set();
for (let proto = WebGL2RenderingContext.prototype; proto && proto !== Object.prototype; proto = Object.getPrototypeOf(proto)) {
  for (const [name, descriptor] of Object.entries(Object.getOwnPropertyDescriptors(proto))) {
    if (name !== 'constructor' && typeof descriptor.value === 'function') methods.add(name);
  }
}
for (const name of methods) {
  const original = WebGL2RenderingContext.prototype[name];
  if (!original) continue;
  const stats = profile.calls[name] = { count: 0, total: 0, max: 0 };
  WebGL2RenderingContext.prototype[name] = function (...args) {
    const start = performance.now();
    try { return Reflect.apply(original, this, args); }
    finally {
      const duration = performance.now() - start;
      stats.count++; stats.total += duration; stats.max = Math.max(stats.max, duration);
      if (duration > 20 && profile.slow.length < 200) profile.slow.push({ name, start, duration });
    }
  };
}
let previous;
function frame(now) {
  if (previous !== undefined) {
    const duration = now - previous;
    profile.frames++;
    profile.maxGap = Math.max(profile.maxGap, duration);
    if (duration > 100 && profile.gaps.length < 200) profile.gaps.push({ start: previous, duration });
  }
  previous = now;
  window.requestAnimationFrame(frame);
}
window.requestAnimationFrame(frame);

// Separate long application callbacks from time spent inside the graphics API.
for (const name of ['requestAnimationFrame', 'setTimeout', 'setInterval']) {
  const original = window[name];
  window[name] = function (callback, ...args) {
    if (typeof callback !== 'function') return Reflect.apply(original, window, [callback, ...args]);
    return Reflect.apply(original, window, [function (...values) {
      const start = performance.now();
      try { return Reflect.apply(callback, this, values); }
      finally {
        const duration = performance.now() - start;
        if (duration > 50 && profile.slow.length < 200) profile.slow.push({ name, start, duration });
      }
    }, ...args]);
  };
}
