/** Derive a color with sufficient contrast against the viewer background. */
export function readableAccent(color: string, dark: boolean, minimum = 4.5): string {
  const match = /^#([\da-f]{6})$/i.exec(color);
  const hex = match?.[1] ?? 'c98c5a';
  const source = [0, 2, 4].map((index) => parseInt(hex.slice(index, index + 2), 16));
  const background = dark ? [32, 33, 36] : [247, 247, 248];
  const luminance = (rgb: number[]) =>
    rgb.reduce((sum, channel, index) => {
      const value = channel / 255;
      return (
        sum +
        (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4) *
          [0.2126, 0.7152, 0.0722][index]
      );
    }, 0);
  const bg = luminance(background);
  const contrast = (rgb: number[]) => {
    const value = luminance(rgb);
    return (Math.max(value, bg) + 0.05) / (Math.min(value, bg) + 0.05);
  };
  const target = dark ? 255 : 0;
  let low = 0;
  let high = 1;
  for (let i = 0; i < 24; i++) {
    const mid = (low + high) / 2;
    const candidate = source.map((channel) => Math.round(channel + (target - channel) * mid));
    if (contrast(candidate) >= minimum) high = mid;
    else low = mid;
  }
  const result =
    contrast(source) >= minimum
      ? source
      : source.map((channel) => Math.round(channel + (target - channel) * high));
  return `#${result.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`;
}
