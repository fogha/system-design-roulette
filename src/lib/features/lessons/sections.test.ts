import { describe, expect, it } from 'vitest';
import { CALLOUTS, LESSON_SECTIONS, identify, parseHint } from './sections';
import { iconSvg } from './icons';

describe('lesson sections', () => {
  it('reads a reading hint in its common spellings', () => {
    for (const text of [
      '~3 min · Read closely: the exercise builds on this.',
      '≈3 minutes — Read closely: the exercise builds on this.',
      '3 mins: Read closely: the exercise builds on this.',
    ]) {
      const hint = parseHint(text);
      expect(hint?.minutes, text).toBe(3);
      expect(hint?.text, text).toMatch(/^Read closely/);
    }
    expect(parseHint('Key idea: not a hint')).toBeNull();
    expect(parseHint('~3 min')).toBeNull();
  });

  it('knows every canonical section by title, whatever its case', () => {
    for (const section of LESSON_SECTIONS) {
      expect(identify(section.title.toUpperCase())?.icon).toBe(section.icon);
    }
    expect(identify('Interview framing')).toBeNull();
  });

  it('has icon data for every section and callout it draws', () => {
    for (const section of LESSON_SECTIONS) expect(iconSvg(section.icon)).toContain('<svg');
    for (const callout of Object.values(CALLOUTS)) expect(iconSvg(callout.icon)).toContain('<svg');
    expect(iconSvg('clock')).toContain('<svg');
    expect(iconSvg('no-such-icon')).toBe('');
  });
});
