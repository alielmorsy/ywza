/** "avc1.PPCCLL" from the SPS NAL (type 7) of an Annex-B keyframe. */
export function avcCodec(b: Uint8Array): string | null {
  for (let i = 0; i + 6 < b.length; i++)
    if (b[i] === 0 && b[i + 1] === 0 && b[i + 2] === 1 && (b[i + 3] & 0x1f) === 7)
      return "avc1." + [b[i + 4], b[i + 5], b[i + 6]].map((x) => x.toString(16).padStart(2, "0")).join("");
  return null;
}

interface FrameHeader { pts_us: number; keyframe: boolean; width: number; height: number }

/** Decodes live messages (u32le header length, `FrameHeader` JSON, Annex-B H.264). `onFrame` must not keep the frame. */
export function createDecoder(onFrame: (f: VideoFrame) => void) {
  let dec: VideoDecoder | null = null;
  let cfg = "";
  const close = () => { if (dec && dec.state !== "closed") dec.close(); dec = null; cfg = ""; };
  return {
    close,
    push(buf: ArrayBuffer) {
      const len = new DataView(buf).getUint32(0, true);
      const h = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 4, len))) as FrameHeader;
      const data = new Uint8Array(buf, 4 + len);
      const codec = h.keyframe && avcCodec(data), key = `${codec} ${h.width}x${h.height}`;
      if (codec && key !== cfg) { // first keyframe, or new size/profile (monitor switch)
        close();
        dec = new VideoDecoder({
          output: (f) => { try { onFrame(f); } finally { f.close(); } },
          error: (e) => { console.warn("live decode:", e); cfg = ""; }, // rebuilt on the next keyframe
        });
        dec.configure({ codec, codedWidth: h.width, codedHeight: h.height, optimizeForLatency: true }); // no description = Annex-B
        cfg = key;
      }
      if (dec?.state !== "configured") return; // waiting for a keyframe
      dec.decode(new EncodedVideoChunk({ type: h.keyframe ? "key" : "delta", timestamp: h.pts_us, data }));
    },
  };
}
