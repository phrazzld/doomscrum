import wave
import math
import random

SAMPLE_RATE = 44100
DURATION = 60.0  # seconds
NUM_SAMPLES = int(SAMPLE_RATE * DURATION)

# 120 BPM -> 0.5 seconds per beat
BEAT_LEN = 0.5

# Prepare PCM buffer
left_channel = [0.0] * NUM_SAMPLES
right_channel = [0.0] * NUM_SAMPLES

for s in range(NUM_SAMPLES):
    t = s / SAMPLE_RATE
    
    # 4/4 Beat grid (120 BPM)
    beat_idx = int(t / BEAT_LEN)
    t_in_beat = t % BEAT_LEN
    
    # --- Drum: Kick ---
    # Kick on every beat
    kick_val = 0.0
    if t_in_beat < 0.18:
        # pitch sweep from 140Hz to 45Hz
        freq = 140.0 - 95.0 * (t_in_beat / 0.18)
        # exponential decay envelope
        env = math.exp(-t_in_beat * 18.0)
        kick_val = math.sin(2 * math.pi * freq * t_in_beat) * env * 0.45

    # --- Drum: Hat ---
    # Offbeat hi-hat (0.25 offset)
    hat_val = 0.0
    t_in_hat = (t_in_beat - 0.25) % BEAT_LEN
    if t_in_hat < 0.05 and t > 0.25:
        # white noise Hihat with decay
        env = math.exp(-t_in_hat * 45.0)
        hat_val = (random.random() * 2.0 - 1.0) * env * 0.08
        
    # --- Synthesizer: Bassline ---
    # 4 bars chord progression (A minor -> G major -> F major -> E major)
    # A bar is 4 beats = 2.0s
    bar_idx = int(t / 2.0)
    chord_seq = [55.0, 48.99, 43.65, 41.20]  # A1, G1, F1, E1
    chord_freq = chord_seq[bar_idx % len(chord_seq)]
    
    # 1/8 note synth pluck: cycle 0.25s
    t_pluck = t % 0.25
    pluck_val = 0.0
    if t_pluck < 0.18:
        env_pluck = math.exp(-t_pluck * 14.0)
        # sawtooth wave approximated via harmonics or formula
        # let's use sum of a couple sines for safety to avoid high freq aliasing
        pluck_val = (
            math.sin(2 * math.pi * chord_freq * t_pluck) +
            0.5 * math.sin(2 * math.pi * (chord_freq * 2) * t_pluck) +
            0.25 * math.sin(2 * math.pi * (chord_freq * 3) * t_pluck)
        ) * env_pluck * 0.12
        
    left_channel[s] = kick_val + hat_val + pluck_val
    right_channel[s] = kick_val - hat_val + pluck_val * 0.9  # slight width

# Write to Stereo WAV
with wave.open("demo/public/sfx/loop.wav", "w") as wav_file:
    wav_file.setnchannels(2)
    wav_file.setsampwidth(2)
    wav_file.setframerate(SAMPLE_RATE)
    
    # Mix and clip
    samples = []
    for l, r in zip(left_channel, right_channel):
        l_clamped = max(-1.0, min(1.0, l))
        r_clamped = max(-1.0, min(1.0, r))
        
        # Convert to 16-bit PCM scale
        l_val = int(l_clamped * 32767)
        r_val = int(r_clamped * 32767)
        
        samples.extend([l_val, r_val])
        
    # Write frames
    import struct
    data = struct.pack(f"<{len(samples)}h", *samples)
    wav_file.writeframes(data)
