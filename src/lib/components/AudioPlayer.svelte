<script lang="ts">
  /**
   * Two-host audio lesson player. Engine 'vibevoice' plays rendered per-line
   * files via the asset protocol; engine 'speech' uses the webview's native
   * speechSynthesis with two distinct system voices. Zero network either way.
   *
   * Two well-documented Web Speech API foot-guns matter here because lines
   * run ~25-45 words (~150-200 chars) each:
   * - `getVoices()` loads asynchronously in most engines; calling it inline
   *   before `voiceschanged` fires returns `[]`, so both hosts silently
   *   collapse onto the same default voice.
   * - Chromium/WebKit reliably stall or drop `onend` on single utterances
   *   once they run long (~15s / ~200-250 chars), which would otherwise
   *   leave `playing` stuck true with silence and no way to recover except
   *   manually skipping.
   * Both are handled below: voices are awaited once and cached, long lines
   * are split into short chunks spoken back-to-back, and every chunk gets a
   * watchdog timer that force-advances if the engine never fires
   * onend/onerror.
   */
  import { onDestroy } from 'svelte';
  import { isTauri } from '../ipc';
  import { chunkText } from '../audio-chunks';
  import { SkipBack, SkipForward, Play, Pause, GraduationCap, Presentation, AudioLines } from 'lucide-svelte';

  interface Line {
    speaker: string;
    text: string;
    file?: string | null;
  }
  let { lines, engine }: { lines: Line[]; engine: string } = $props();

  let idx = $state(0);
  let playing = $state(false);
  let rate = $state(1.0);
  const RATES = [0.8, 1.0, 1.2, 1.5];

  let audioEl: HTMLAudioElement | null = null;
  let convertFileSrc: ((p: string) => string) | null = null;
  if (isTauri) {
    import('@tauri-apps/api/core').then((m) => (convertFileSrc = m.convertFileSrc));
  }

  // Bumped by every user action that interrupts playback (pause, skip, rate
  // change, destroy) so stale onend/watchdog callbacks from the previous
  // utterance can recognize they've been superseded and do nothing.
  let playToken = 0;
  let watchdog: ReturnType<typeof setTimeout> | null = null;

  function clearWatchdog() {
    if (watchdog) {
      clearTimeout(watchdog);
      watchdog = null;
    }
  }

  function interrupt() {
    playToken += 1;
    clearWatchdog();
    speechSynthesis.cancel();
    audioEl?.pause();
  }

  let teacherVoice: SpeechSynthesisVoice | null = null;
  let studentVoice: SpeechSynthesisVoice | null = null;
  let voicesPromise: Promise<void> | null = null;

  function loadVoiceList(): Promise<SpeechSynthesisVoice[]> {
    return new Promise((resolve) => {
      const existing = speechSynthesis.getVoices();
      if (existing.length > 0) {
        resolve(existing);
        return;
      }
      let settled = false;
      const done = () => {
        if (settled) return;
        settled = true;
        resolve(speechSynthesis.getVoices());
      };
      speechSynthesis.addEventListener('voiceschanged', done, { once: true });
      // Some engines never fire voiceschanged — don't block playback forever.
      setTimeout(done, 1500);
    });
  }

  function ensureVoices(): Promise<void> {
    if (!voicesPromise) {
      voicesPromise = loadVoiceList().then((list) => {
        const all = list.filter((v) => v.lang.startsWith('en'));
        const pick = (names: string[]) => all.find((v) => names.some((n) => v.name.includes(n))) ?? null;
        teacherVoice = pick(['Daniel', 'Aaron', 'Alex']) ?? all[0] ?? null;
        studentVoice = pick(['Samantha', 'Karen', 'Moira']) ?? all[1] ?? all[0] ?? null;
      });
    }
    return voicesPromise;
  }

  /** ~14 chars/sec at 1x is a normal speaking pace; give generous headroom
   * so the watchdog never preempts a chunk that's simply still talking. */
  function watchdogMs(text: string): number {
    const estimate = ((text.length / 14) * 1000) / rate;
    return Math.min(Math.max(estimate, 2500) + 4000, 25000);
  }

  function playLine(i: number) {
    if (i >= lines.length) {
      playing = false;
      idx = lines.length - 1;
      return;
    }
    idx = i;
    const line = lines[i];
    const token = playToken;
    if (engine === 'vibevoice' && line.file && convertFileSrc) {
      audioEl?.pause();
      audioEl = new Audio(convertFileSrc(line.file));
      audioEl.playbackRate = rate;
      audioEl.onended = () => {
        if (token === playToken && playing) playLine(i + 1);
      };
      audioEl.onerror = () => {
        if (token === playToken) speakLine(i, token);
      };
      audioEl.play().catch(() => {
        if (token === playToken) speakLine(i, token);
      });
    } else {
      speakLine(i, token);
    }
  }

  async function speakLine(i: number, token: number) {
    await ensureVoices();
    if (token !== playToken) return; // superseded while voices were loading
    const line = lines[i];
    const chunks = chunkText(line.text).filter(Boolean);
    if (chunks.length === 0) {
      if (playing) playLine(i + 1);
      return;
    }
    const voice = line.speaker === 'teacher' ? teacherVoice : studentVoice;
    speakChunks(chunks, 0, voice, i, token);
  }

  function speakChunks(
    chunks: string[],
    chunkIdx: number,
    voice: SpeechSynthesisVoice | null,
    lineIdx: number,
    token: number
  ) {
    if (token !== playToken) return;
    if (chunkIdx >= chunks.length) {
      if (playing) playLine(lineIdx + 1);
      return;
    }
    const u = new SpeechSynthesisUtterance(chunks[chunkIdx]);
    u.voice = voice;
    u.rate = rate;
    let settled = false;
    const advance = () => {
      if (settled || token !== playToken) return;
      settled = true;
      clearWatchdog();
      speakChunks(chunks, chunkIdx + 1, voice, lineIdx, token);
    };
    u.onend = advance;
    u.onerror = advance;
    clearWatchdog();
    watchdog = setTimeout(advance, watchdogMs(chunks[chunkIdx]));
    speechSynthesis.speak(u);
  }

  function toggle() {
    if (playing) {
      playing = false;
      interrupt();
    } else {
      playing = true;
      playLine(idx);
    }
  }

  function skip(d: number) {
    const next = Math.min(Math.max(idx + d, 0), lines.length - 1);
    interrupt();
    if (playing) playLine(next);
    else idx = next;
  }

  function cycleRate() {
    rate = RATES[(RATES.indexOf(rate) + 1) % RATES.length];
    if (playing) {
      interrupt();
      playLine(idx); // restart current line at the new rate
    }
  }

  onDestroy(() => {
    interrupt();
  });
</script>

<div class="player">
  <div class="controls">
    <button class="pbtn mono" onclick={() => skip(-1)} aria-label="previous line"><SkipBack size={12} /></button>
    <button class="pbtn main mono" onclick={toggle}>{#if playing}<Pause size={13} />{:else}<Play size={13} />{/if}</button>
    <button class="pbtn mono" onclick={() => skip(1)} aria-label="next line"><SkipForward size={12} /></button>
    <button class="pbtn mono rate" onclick={cycleRate}>{rate}×</button>
  </div>
  <div class="now">
    <span class="who mono" class:student={lines[idx]?.speaker === 'student'}>
      {#if lines[idx]?.speaker === 'student'}<GraduationCap size={11} /> student{:else}<Presentation size={11} /> teacher{/if}
    </span>
    <span class="line-text">{lines[idx]?.text}</span>
  </div>
  <div class="meta mono">
    <span>{idx + 1}/{lines.length}</span>
    <span class="eng"><AudioLines size={10} /> {engine === 'vibevoice' ? 'vibevoice' : 'system speech'}</span>
  </div>
</div>

<style>
  .player {
    display: flex;
    align-items: center;
    gap: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    padding: 8px 14px;
    margin: 14px 24px 0;
  }
  .controls {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }
  .pbtn {
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--fg);
    border-radius: var(--radius-control);
    font-size: 12px;
    padding: 5px 10px;
    cursor: pointer;
  }
  .pbtn.main {
    border-color: var(--accent);
    color: var(--accent);
    font-size: 13px;
  }
  .pbtn.rate {
    font-size: 10px;
    color: var(--muted);
  }
  .pbtn:hover {
    border-color: var(--muted);
  }
  .now {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .who {
    font-size: 10px;
    color: var(--accent);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .who.student {
    color: var(--muted);
  }
  .line-text {
    font-size: 13px;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: 10px;
    color: var(--faint);
    display: flex;
    gap: 12px;
    flex-shrink: 0;
  }
</style>
