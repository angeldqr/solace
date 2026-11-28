import { listen } from "@tauri-apps/api/event";
import { useSettings } from "../settings/settings";

export type Chime = "needsYou" | "finished";

/** Notes (Hz) and their offsets (s): rising for "needs you", a soft falling pair for "done". */
const NOTES: Record<Chime, [number, number][]> = {
  needsYou: [
    [659.25, 0],
    [987.77, 0.12],
  ],
  finished: [
    [783.99, 0],
    [523.25, 0.14],
  ],
};

let ctx: AudioContext | null = null;

/** A short synthesized chime: no audio files to ship, and it follows the volume setting. */
export function playChime(chime: Chime, volume = useSettings.getState().soundVolume) {
  if (volume <= 0) return;
  ctx ??= new AudioContext();
  // WebView2 starts audio suspended until the page has seen a click or a key.
  void ctx.resume();
  const now = ctx.currentTime;
  const peak = (volume / 100) * 0.25;
  for (const [freq, at] of NOTES[chime]) {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.value = freq;
    gain.gain.setValueAtTime(0, now + at);
    gain.gain.linearRampToValueAtTime(peak, now + at + 0.01);
    gain.gain.exponentialRampToValueAtTime(0.0001, now + at + 0.45);
    osc.connect(gain).connect(ctx.destination);
    osc.start(now + at);
    osc.stop(now + at + 0.5);
  }
}

let started = false;

/** Plays the chime for each notice the backend sends (agents the user isn't looking at). */
export function startSounds() {
  if (started) return;
  started = true;
  void listen<Chime>("notice", (event) => {
    const s = useSettings.getState();
    const on = event.payload === "needsYou" ? s.soundNeedsYou : s.soundFinished;
    if (on) playChime(event.payload);
  });
}
