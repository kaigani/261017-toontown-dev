# Spec issues

Ambiguities, contradictions or gaps found in `spec/` during implementation. One entry per issue:
section, quote, problem, decision taken (ADR link) or question for the spec owner.

## SI-001 — 09 §4.4 "≈ CRF 18 equivalent" under platform H.264 encoders (found in C0.4; open)

- **Section:** 09 §4.4 Video settings.
- **Quote:** "Quality | high (≈ CRF 18 equivalent) | low / medium / high / lossless where supported, or numeric
  0–100".
- **Problem:** user decision 3 (plan §6) ships only platform H.264 encoders (`h264_mf`, `h264_videotoolbox`) in an LGPL
  FFmpeg sidecar. CRF is a rate-control mode of x264, which is GPL and excluded. Neither platform encoder implements
  CRF, so "≈ CRF 18 equivalent" can only be met as a quality target, not as x264 CRF semantics.
- **Decision taken (ADR-0008):** M10 C10.1 maps low / medium / high / 0–100 to each encoder's quality-based mode
  (Media Foundation quality, VideoToolbox constant quality or bit rate) and measures the mapping in experiment
  E-FF1.
- **Question for the spec owner (open):** confirm that "≈ CRF 18 equivalent" means a visually comparable quality
  target.
- **Settled by ruling C04-R1-03 (not part of the open question):** "lossless where supported" is met by PNG image
  sequences and by FFV1 (FFmpeg's native LGPL encoder) in MKV, and in MOV once verified. ProRes variants, 4444
  included, are labelled "visually lossless". Lossless output is verified in M10 by decoded-pixel comparison against
  the rendered frames (ADR-0008 §1).
