(() => {
  const initHeroShader = () => {
    const canvas = document.querySelector(".hero-shader");
    if (!canvas) return;

    const gl = canvas.getContext("webgl2", {
      alpha: true,
      antialias: false,
      depth: false,
      premultipliedAlpha: true,
      preserveDrawingBuffer: false,
      stencil: false,
    });
    if (!gl) return;

    const vertexSource = `#version 300 es
in vec2 position;

void main() {
  gl_Position = vec4(position, 0.0, 1.0);
}`;

    const fragmentSource = `#version 300 es
precision highp float;

uniform vec2 u_size;
uniform float u_inverse_dpr;
uniform float u_stripe_max_index;
uniform float u_stripe_width;
uniform float u_inverse_stripe_width;
uniform float u_gradient_top;
uniform float u_gradient_vertical_span;
uniform float u_inverse_gradient_denominator;
uniform float u_wave_phase;
uniform float u_secondary_wave_phase;
uniform float u_intro_active;
uniform float u_intro_elapsed;
uniform float u_shine_progress;

uniform vec3 u_start_color;
uniform vec3 u_highlight_delta;
uniform float u_alpha;
uniform float u_intro_alpha;
uniform float u_speed_up_shine_boost;
uniform float u_grain_alpha;
uniform float u_grain_luminance;
uniform float u_grain_contrast;
uniform float u_grain_saturation;

uniform float u_gradient_band_width;
uniform float u_gradient_min_stop;
uniform float u_gradient_max_stop;
uniform float u_intro_reveal_duration;
uniform float u_intro_reveal_duration_inverse;
uniform float u_intro_idle_blend_duration_inverse;
uniform float u_intro_stagger;
uniform float u_intro_center_index;
uniform float u_intro_center_distance;
uniform float u_intro_start_center;
uniform float u_intro_idle_center;
uniform float u_idle_stripe_phase;
uniform float u_idle_secondary_stripe_phase;
uniform float u_idle_primary_amplitude;
uniform float u_idle_secondary_amplitude;

const float GRAIN_SIZE = 180.0;
const vec2 NOISE_HASH_SCALE = vec2(127.1, 311.7);
const float NOISE_HASH_OFFSET = 43758.5;
const mat3x2 NOISE_COLOR_OFFSETS = mat3x2(
  vec2(17.0, 3.0),
  vec2(7.0, 29.0),
  vec2(31.0, 11.0)
);

out vec4 fragment_color;

float clamp01(float value) {
  return clamp(value, 0.0, 1.0);
}

float easeOutCubic(float value) {
  float inverse = 1.0 - value;
  return 1.0 - inverse * inverse * inverse;
}

float introRevealProgress(float delayed_elapsed) {
  if (u_intro_active < 0.5) return 1.0;
  return easeOutCubic(clamp01(delayed_elapsed * u_intro_reveal_duration_inverse));
}

float introIdleProgress(float delayed_elapsed) {
  if (u_intro_active < 0.5) return 1.0;
  return easeOutCubic(clamp01(
    (delayed_elapsed - u_intro_reveal_duration) *
    u_intro_idle_blend_duration_inverse
  ));
}

float hashNoise(vec2 value) {
  return fract(sin(dot(value, NOISE_HASH_SCALE)) * NOISE_HASH_OFFSET);
}

vec3 overlayBlend(vec3 base, vec3 blend) {
  vec3 low = 2.0 * base * blend;
  vec3 high = 1.0 - 2.0 * (1.0 - base) * (1.0 - blend);
  return mix(low, high, step(vec3(0.5), base));
}

float gradientHighlightAmount(
  float position,
  float band_start,
  float center,
  float band_end
) {
  if (position < band_start) return 1.0 - clamp01(position / band_start);
  if (position < center) return clamp01((position - band_start) / (center - band_start));
  if (position < band_end) return 1.0 - clamp01((position - center) / (band_end - center));
  return 0.0;
}

void main() {
  vec2 css_position = vec2(
    gl_FragCoord.x * u_inverse_dpr,
    u_size.y - gl_FragCoord.y * u_inverse_dpr
  );
  float stripe_index = min(
    floor(css_position.x * u_inverse_stripe_width),
    u_stripe_max_index
  );
  float stripe_start = stripe_index * u_stripe_width;
  float reveal_delay = max(
    0.0,
    abs(stripe_index - u_intro_center_index) - u_intro_center_distance
  ) * u_intro_stagger;
  float delayed_intro_elapsed = u_intro_elapsed - reveal_delay;
  float reveal_progress = introRevealProgress(delayed_intro_elapsed);
  float idle_progress = introIdleProgress(delayed_intro_elapsed);
  float alpha = u_intro_alpha + (u_alpha - u_intro_alpha) * idle_progress;
  float stripe_phase = stripe_index * u_idle_stripe_phase;
  float secondary_stripe_phase = stripe_index * u_idle_secondary_stripe_phase;
  float primary_wave = sin(u_wave_phase - stripe_phase);
  float secondary_wave = sin(u_secondary_wave_phase + secondary_stripe_phase);
  float idle_center = 0.5 +
    primary_wave * u_idle_primary_amplitude +
    secondary_wave * u_idle_secondary_amplitude;
  float intro_center = mix(u_intro_start_center, u_intro_idle_center, reveal_progress);
  float center = mix(intro_center, idle_center, idle_progress);
  float band_start = clamp(
    center - u_gradient_band_width,
    u_gradient_min_stop,
    u_gradient_max_stop
  );
  float band_end = clamp(
    center + u_gradient_band_width,
    u_gradient_min_stop,
    u_gradient_max_stop
  );
  vec2 gradient_offset = vec2(
    css_position.x - stripe_start,
    css_position.y - u_gradient_top
  );
  vec2 gradient_axis = vec2(u_stripe_width, u_gradient_vertical_span);
  float gradient_position = clamp01(
    dot(gradient_offset, gradient_axis) * u_inverse_gradient_denominator
  );
  vec3 gradient = u_start_color + u_highlight_delta * gradientHighlightAmount(
    gradient_position,
    band_start,
    center,
    band_end
  );
  float gradient_alpha = clamp01(
    alpha * (1.0 + u_shine_progress * u_speed_up_shine_boost)
  ) * reveal_progress;
  float out_alpha = gradient_alpha + alpha * (1.0 - gradient_alpha);
  vec3 color = (
    gradient * gradient_alpha +
    u_start_color * alpha * (1.0 - gradient_alpha)
  ) / max(out_alpha, 0.0001);

  vec2 grain_position = floor(mod(css_position, GRAIN_SIZE));
  float luminance = u_grain_luminance +
    (hashNoise(grain_position) - 0.5) * u_grain_contrast;
  vec3 color_shift = vec3(
    hashNoise(grain_position + NOISE_COLOR_OFFSETS[0]) - 0.5,
    hashNoise(grain_position + NOISE_COLOR_OFFSETS[1]) - 0.5,
    hashNoise(grain_position + NOISE_COLOR_OFFSETS[2]) - 0.5
  ) * u_grain_saturation;
  vec3 grain = clamp((vec3(luminance) + color_shift) / 255.0, 0.0, 1.0);
  color = mix(color, overlayBlend(color, grain), u_grain_alpha);

  fragment_color = vec4(color * out_alpha, out_alpha);
}`;

    const compileShader = (type, source) => {
      const shader = gl.createShader(type);
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        console.warn(gl.getShaderInfoLog(shader));
        gl.deleteShader(shader);
        return null;
      }
      return shader;
    };

    const vertexShader = compileShader(gl.VERTEX_SHADER, vertexSource);
    const fragmentShader = compileShader(gl.FRAGMENT_SHADER, fragmentSource);
    if (!vertexShader || !fragmentShader) return;

    const program = gl.createProgram();
    gl.attachShader(program, vertexShader);
    gl.attachShader(program, fragmentShader);
    gl.linkProgram(program);
    gl.deleteShader(vertexShader);
    gl.deleteShader(fragmentShader);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      console.warn(gl.getProgramInfoLog(program));
      gl.deleteProgram(program);
      return;
    }

    const position = gl.getAttribLocation(program, "position");
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 3, -1, -1, 3]),
      gl.STATIC_DRAW
    );
    gl.enableVertexAttribArray(position);
    gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
    gl.useProgram(program);

    const locations = new Map();
    const uniform = (name) => {
      if (!locations.has(name)) locations.set(name, gl.getUniformLocation(program, name));
      return locations.get(name);
    };
    const set1 = (name, value) => gl.uniform1f(uniform(name), value);
    const set2 = (name, x, y) => gl.uniform2f(uniform(name), x, y);
    const set3 = (name, x, y, z) => gl.uniform3f(uniform(name), x, y, z);

    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let width = 1;
    let height = 1;
    let dpr = 1;
    let stripeCount = 1;
    let stripeWidth = 1;
    let frame = 0;
    let previousTime = null;
    let wavePhase = Math.random() * Math.PI * 2;
    let secondaryWavePhase = wavePhase * -0.7;
    // Smooth half-sine envelope timed from click; null when idle.
    let pulseStartedAt = null;
    const PULSE_DURATION_MS = 1100;
    const introStartedAt = performance.now();

    const pulseWave = () => {
      if (reduceMotion.matches) return;
      pulseStartedAt = performance.now();
    };

    const pulseEnvelope = (time) => {
      if (pulseStartedAt === null) return 0;
      const t = (time - pulseStartedAt) / PULSE_DURATION_MS;
      if (t >= 1) {
        pulseStartedAt = null;
        return 0;
      }
      // Soft attack + release (no hard edges on speed / amplitude).
      return Math.sin(Math.PI * Math.min(1, Math.max(0, t)));
    };

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      width = Math.max(1, rect.width);
      height = Math.max(1, rect.height);
      dpr = Math.max(1, window.devicePixelRatio || 1);
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      gl.viewport(0, 0, canvas.width, canvas.height);

      const minCount = Math.max(1, Math.ceil(width / 120));
      const maxCount = Math.max(1, Math.floor(width / 64));
      const targetCount = Math.max(1, Math.round(width / 110));
      stripeCount = Math.max(minCount, Math.min(maxCount, targetCount));
      stripeWidth = width / stripeCount;
    };

    const render = (time) => {
      const reduced = reduceMotion.matches;
      const delta = reduced || previousTime === null
        ? 0
        : Math.min((time - previousTime) / 1000, 0.064);
      previousTime = reduced ? null : time;

      const pulse = pulseEnvelope(time);
      // Idle crawl vs a rounded surge that still sweeps ~2π across the stripes.
      wavePhase += (0.42 + pulse * 9.5) * delta;
      secondaryWavePhase += (0.26 + pulse * 6) * delta;

      const verticalSpan = height * 1.7;
      set2("u_size", width, height);
      set1("u_inverse_dpr", 1 / dpr);
      set1("u_stripe_max_index", stripeCount - 1);
      set1("u_stripe_width", stripeWidth);
      set1("u_inverse_stripe_width", 1 / stripeWidth);
      set1("u_gradient_top", height * -0.35);
      set1("u_gradient_vertical_span", verticalSpan);
      set1(
        "u_inverse_gradient_denominator",
        1 / (stripeWidth * stripeWidth + verticalSpan * verticalSpan)
      );
      set1("u_wave_phase", wavePhase);
      set1("u_secondary_wave_phase", secondaryWavePhase);
      set1("u_intro_active", reduced ? 0 : 1);
      set1("u_intro_elapsed", reduced ? 0 : time - introStartedAt - 700);
      set1("u_shine_progress", pulse);

      // Helium's shader palette, shifted into a restrained graphite navy.
      set3("u_start_color", 0.031, 0.047, 0.078);
      set3("u_highlight_delta", 0.082, 0.125, 0.196);
      set1("u_alpha", 0.7);
      set1("u_intro_alpha", 1);
      set1("u_speed_up_shine_boost", 0.28);
      set1("u_grain_alpha", 0.15);
      set1("u_grain_luminance", 144);
      set1("u_grain_contrast", 64);
      set1("u_grain_saturation", 32);

      set1("u_gradient_band_width", 0.48);
      set1("u_gradient_min_stop", 0.14);
      set1("u_gradient_max_stop", 0.96);
      set1("u_intro_reveal_duration", 840);
      set1("u_intro_reveal_duration_inverse", 1 / 840);
      set1("u_intro_idle_blend_duration_inverse", 1 / 500);
      set1("u_intro_stagger", 45);
      set1("u_intro_center_index", (stripeCount - 1) / 2);
      set1("u_intro_center_distance", stripeCount % 2 === 0 ? 0.5 : 0);
      set1("u_intro_start_center", 0.96);
      set1("u_intro_idle_center", 0.5);
      set1("u_idle_stripe_phase", 0.74);
      set1("u_idle_secondary_stripe_phase", 1.28);
      set1("u_idle_primary_amplitude", 0.19 + pulse * 0.1);
      set1("u_idle_secondary_amplitude", 0.055 + pulse * 0.035);

      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      canvas.classList.add("is-ready");

      if (!reduced) frame = requestAnimationFrame(render);
    };

    const restart = () => {
      cancelAnimationFrame(frame);
      previousTime = null;
      render(performance.now());
    };
    const resizeObserver = new ResizeObserver(() => {
      resize();
      restart();
    });
    resizeObserver.observe(canvas);
    reduceMotion.addEventListener("change", restart);
    resize();
    render(performance.now());

    return { pulseWave };
  };

  const heroShader = initHeroShader();

  if (heroShader) {
    document.querySelectorAll('a.btn[href*="releases/latest"]').forEach((link) => {
      link.addEventListener("click", () => heroShader.pulseWave());
    });
  }

  const detectBrowser = () => {
    const ua = navigator.userAgent;
    const brands = navigator.userAgentData?.brands?.map((b) => b.brand.toLowerCase()) ?? [];
    const hasBrand = (name) => brands.some((brand) => brand.includes(name));

    if (
      typeof navigator.brave?.isBrave === "function" ||
      hasBrand("brave") ||
      /\bBrave\b/i.test(ua)
    ) {
      return "brave";
    }
    if (hasBrand("opera") || /OPR\/|Opera/i.test(ua)) return "opera";
    if (hasBrand("yandex") || /YaBrowser\//i.test(ua)) return "yandex";
    if (hasBrand("edge") || /Edg\//i.test(ua)) return "edge";
    if (hasBrand("firefox") || /Firefox\//i.test(ua)) return "firefox";
    // Safari must come after Chromium forks — they all include "Safari" in the UA.
    if (
      /Safari\//i.test(ua) &&
      !/Chrome\/|Chromium\/|Edg\/|OPR\/|YaBrowser\//i.test(ua)
    ) {
      return "safari";
    }
    if (hasBrand("chrome") || /Chrome\//i.test(ua)) return "chrome";
    return null;
  };

  const PINNED_BROWSERS = new Set(["chrome", "edge", "firefox"]);
  const EXTRA_BROWSERS = {
    opera: { title: "Opera", src: "assets/taskbar-opera.svg" },
    brave: { title: "Brave", src: "assets/taskbar-brave.svg" },
    yandex: { title: "Yandex Browser", src: "assets/taskbar-yandex.svg" },
    safari: { title: "Safari", src: "assets/taskbar-safari.svg" },
  };

  const markOpenBrowser = () => {
    const browser = detectBrowser();
    if (!browser) return;

    if (PINNED_BROWSERS.has(browser)) {
      document
        .querySelector(`.win-pin-app[data-browser="${browser}"]`)
        ?.classList.add("using");
      return;
    }

    const meta = EXTRA_BROWSERS[browser];
    const firefoxPin = document.querySelector('.win-pin-app[data-browser="firefox"]');
    if (!meta || !firefoxPin) return;

    const extra = document.createElement("span");
    extra.className = "win-pin using win-pin-app";
    extra.dataset.browser = browser;
    extra.title = meta.title;

    const img = document.createElement("img");
    img.src = meta.src;
    img.alt = "";
    extra.appendChild(img);

    firefoxPin.after(extra);
  };

  markOpenBrowser();

  const header = document.querySelector(".site-header");
  const toggle = document.querySelector(".nav-toggle");
  const mobileNav = document.querySelector("#mobile-nav");

  const onScroll = () => {
    if (!header) return;
    header.classList.toggle("is-scrolled", window.scrollY > 8);
  };

  onScroll();
  window.addEventListener("scroll", onScroll, { passive: true });

  if (toggle && mobileNav) {
    toggle.addEventListener("click", () => {
      const open = mobileNav.hasAttribute("hidden");
      if (open) {
        mobileNav.removeAttribute("hidden");
        toggle.setAttribute("aria-expanded", "true");
        toggle.setAttribute("aria-label", "Close menu");
      } else {
        mobileNav.setAttribute("hidden", "");
        toggle.setAttribute("aria-expanded", "false");
        toggle.setAttribute("aria-label", "Open menu");
      }
    });

    mobileNav.querySelectorAll("a").forEach((link) => {
      link.addEventListener("click", () => {
        mobileNav.setAttribute("hidden", "");
        toggle.setAttribute("aria-expanded", "false");
        toggle.setAttribute("aria-label", "Open menu");
      });
    });
  }

  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const reveals = document.querySelectorAll(".reveal");
  const heroVisual = document.querySelector(".hero-visual");
  const desk = document.querySelector(".desk");
  const demoCursor = document.querySelector(".demo-cursor");
  const trayTarget = document.querySelector("[data-tray-target]");
  const popupMock = document.querySelector(".popup-mock");
  let trayDemoPlayed = false;
  let trayDemoBusy = false;

  const scalePopupToFit = () => {
    if (!popupMock || !heroVisual) return;

    const taskbarClearance = 80;
    const topClearance = (header?.getBoundingClientRect().height || 0) + 16;
    const availableHeight = heroVisual.clientHeight - taskbarClearance - topClearance;
    const naturalHeight = popupMock.offsetHeight;
    const scale = naturalHeight > 0
      ? Math.min(1, Math.max(0.55, availableHeight / naturalHeight))
      : 1;

    popupMock.style.setProperty("--popup-scale", scale.toFixed(4));
  };

  scalePopupToFit();
  window.addEventListener("resize", scalePopupToFit, { passive: true });
  new ResizeObserver(scalePopupToFit).observe(heroVisual);

  const setPopupOpen = (open) => {
    if (!popupMock) return;
    popupMock.classList.toggle("is-open", open);
    trayTarget?.setAttribute("aria-expanded", open ? "true" : "false");
  };

  const togglePopup = () => {
    if (!popupMock || trayDemoBusy) return;
    const willOpen = !popupMock.classList.contains("is-open");
    setPopupOpen(willOpen);

    if (!reduceMotion && trayTarget) {
      trayTarget.classList.add("is-clicked");
      window.setTimeout(() => trayTarget.classList.remove("is-clicked"), 180);
    }
  };

  const playTrayDemo = () => {
    if (trayDemoPlayed || !desk || !demoCursor || !trayTarget || !popupMock) return;
    trayDemoPlayed = true;

    if (reduceMotion) {
      setPopupOpen(true);
      return;
    }

    trayDemoBusy = true;

    const deskRect = desk.getBoundingClientRect();
    const targetRect = trayTarget.getBoundingClientRect();
    const startX = deskRect.width * 0.5;
    const startY = deskRect.height * 0.42;
    const endX = targetRect.left - deskRect.left + targetRect.width * 0.35;
    const endY = targetRect.top - deskRect.top + targetRect.height * 0.45;

    demoCursor.style.setProperty("--x", `${startX}px`);
    demoCursor.style.setProperty("--y", `${startY}px`);
    demoCursor.classList.add("is-active");

    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        demoCursor.classList.add("is-moving");
        demoCursor.style.setProperty("--x", `${endX}px`);
        demoCursor.style.setProperty("--y", `${endY}px`);
      });
    });

    window.setTimeout(() => {
      demoCursor.classList.remove("is-moving");
      demoCursor.classList.add("is-at-target");
      demoCursor.classList.add("is-clicking");
      trayTarget.classList.add("is-clicked");
    }, 1180);

    window.setTimeout(() => {
      demoCursor.classList.remove("is-clicking");
      setPopupOpen(true);
    }, 1360);

    window.setTimeout(() => {
      demoCursor.classList.add("is-done");
      trayTarget.classList.remove("is-clicked");
      trayDemoBusy = false;
    }, 1720);
  };

  const onHeroVisualVisible = () => {
    if (reduceMotion) {
      setPopupOpen(true);
      return;
    }
    window.setTimeout(playTrayDemo, 480);
  };

  if (trayTarget) {
    trayTarget.addEventListener("click", togglePopup);
    trayTarget.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      togglePopup();
    });
  }

  if (reduceMotion || !("IntersectionObserver" in window)) {
    reveals.forEach((el) => el.classList.add("is-visible"));
    setPopupOpen(true);
    return;
  }

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add("is-visible");
        if (entry.target === heroVisual) onHeroVisualVisible();
        observer.unobserve(entry.target);
      });
    },
    { rootMargin: "0px 0px -8% 0px", threshold: 0.12 }
  );

  reveals.forEach((el, index) => {
    el.style.transitionDelay = `${Math.min(index % 6, 5) * 40}ms`;
    observer.observe(el);
  });
})();
