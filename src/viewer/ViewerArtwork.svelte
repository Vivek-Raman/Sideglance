<script lang="ts">
  import { onMount } from 'svelte';

  const roots = new Set<SVGSVGElement>();
  let focused = false;
  let reduced = false;

  function syncPlayback() {
    const paused = !focused || document.hidden || reduced;
    for (const root of roots) {
      if (paused) root.pauseAnimations();
      else root.unpauseAnimations();
    }
  }

  function playback(node: SVGSVGElement) {
    roots.add(node);
    syncPlayback();
    return { destroy: () => roots.delete(node) };
  }

  onMount(() => {
    const media = window.matchMedia('(prefers-reduced-motion: reduce)');
    const focus = () => {
      focused = true;
      syncPlayback();
    };
    const blur = () => {
      focused = false;
      syncPlayback();
    };
    const motion = () => {
      reduced = media.matches;
      syncPlayback();
    };
    focused = document.hasFocus();
    motion();
    window.addEventListener('focus', focus);
    window.addEventListener('blur', blur);
    document.addEventListener('visibilitychange', syncPlayback);
    media.addEventListener('change', motion);
    return () => {
      window.removeEventListener('focus', focus);
      window.removeEventListener('blur', blur);
      document.removeEventListener('visibilitychange', syncPlayback);
      media.removeEventListener('change', motion);
    };
  });
</script>

<svg
  use:playback
  class="line-art"
  viewBox="0 0 1000 480"
  preserveAspectRatio="xMaxYMin meet"
  aria-hidden="true"
>
  <g fill="none" stroke="currentColor" stroke-linecap="round" vector-effect="non-scaling-stroke">
    <path
      d="M70-40C190-8 230 124 360 120C520 116 610 10 742 50C880 91 910 168 1060 140"
      stroke-width="2.5"
      ><animate
        attributeName="opacity"
        values="0.55;0.9;0.55"
        keyTimes="0;0.42;1"
        begin="-4s"
        dur="18s"
        repeatCount="indefinite"
      /></path
    >
    <path
      class="line-art-medium"
      d="M190-40C290 19 290 171 420 168C555 165 650 84 780 123C910 165 938 246 1060 236"
      stroke-width="2.5"
      ><animate
        attributeName="opacity"
        values="0.75;0.46;0.75"
        keyTimes="0;0.64;1"
        begin="-11s"
        dur="22s"
        repeatCount="indefinite"
      /></path
    >
    <path
      class="line-art-medium"
      d="M310-40C385 42 380 226 500 220C630 214 706 159 826 201C930 238 968 315 1060 307"
      stroke-width="2.5"
      ><animate
        attributeName="opacity"
        values="0.38;0.76;0.38"
        keyTimes="0;0.31;1"
        begin="-7s"
        dur="20s"
        repeatCount="indefinite"
      /></path
    >
    <path
      class="line-art-extra"
      d="M420-40C478 72 474 283 583 273C694 263 758 213 859 251C944 284 982 357 1055 350"
      stroke-width="2.5"
      ><animate
        attributeName="opacity"
        values="0.62;0.34;0.62"
        keyTimes="0;0.57;1"
        begin="-17s"
        dur="24s"
        repeatCount="indefinite"
      /></path
    >
  </g>
</svg>
<svg
  use:playback
  class="bottom-line-art"
  viewBox="0 0 1000 420"
  preserveAspectRatio="xMidYMax meet"
  aria-hidden="true"
>
  <g fill="none" stroke="currentColor" stroke-linecap="round" vector-effect="non-scaling-stroke">
    <path d="M-60 392C154 326 273 336 460 386C650 437 797 303 1060 365" stroke-width="2.5"
      ><animate
        attributeName="opacity"
        values="0.45;0.72;0.45"
        keyTimes="0;0.48;1"
        begin="-9s"
        dur="20s"
        repeatCount="indefinite"
      /></path
    >
    <path d="M-45 444C176 352 360 378 544 431C745 488 862 361 1050 398" stroke-width="2.5"
      ><animate
        attributeName="opacity"
        values="0.36;0.62;0.36"
        keyTimes="0;0.69;1"
        begin="-15s"
        dur="24s"
        repeatCount="indefinite"
      /></path
    >
  </g>
</svg>

<style>
  .line-art,
  .bottom-line-art {
    position: absolute;
    z-index: 0;
    height: auto;
    color: var(--accent);
    pointer-events: none;
  }
  .line-art {
    top: 0;
    right: 0;
    width: min(880px, 80%);
    aspect-ratio: 1000 / 480;
    opacity: 0.48;
  }
  .bottom-line-art {
    right: 0;
    bottom: 0;
    width: 100%;
    aspect-ratio: 1000 / 420;
    opacity: 0.55;
  }
  @media (max-width: 440px), (max-height: 360px) {
    .line-art-extra {
      display: none;
    }
  }
  @media (max-width: 340px), (max-height: 280px) {
    .line-art-medium {
      display: none;
    }
  }
</style>
