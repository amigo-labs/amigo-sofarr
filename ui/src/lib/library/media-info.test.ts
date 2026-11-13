import { describe, expect, it } from 'vitest';
import { mediaInfoRows } from './media-info';

const read = {
  status: 'complete',
  container: 'matroska',
  video: {
    codec: 'h265',
    encoder: 'x265',
    width: 1920,
    height: 1080,
    dynamic_range: { hdr10: true, hdr10_plus: false, hlg: false, dolby_vision: false },
  },
  audio: [
    { codec: 'eac3', channels: 6, object_audio: false, language: 'ger' },
    { codec: 'ac3', channels: 2, object_audio: false, language: 'eng' },
  ],
  subtitles: [
    { language: 'ger', format: 'srt', forced: true, default: false, hearing_impaired: false },
    { language: 'und', format: 'pgs', forced: false, default: false, hearing_impaired: true },
  ],
  labels: { video: 'x265', audio: 'EAC3', channels: '5.1', dynamic_range: 'HDR10' },
};

describe('mediaInfoRows', () => {
  it('names every part in German with the labels of the file names', () => {
    expect(mediaInfoRows(read)).toEqual([
      { label: 'Gelesen', value: 'vollständig' },
      { label: 'Container', value: 'Matroska' },
      { label: 'Video', value: 'x265 · 1920 × 1080 · HDR10' },
      { label: 'Audio', value: 'EAC3 5.1' },
      { label: 'Audiosprachen', value: 'Deutsch, Englisch' },
      {
        label: 'Untertitel',
        value: 'Deutsch (erzwungen), Sprache unbekannt (normal, für Hörgeschädigte)',
      },
    ]);
  });

  it('says what is missing instead of leaving it out', () => {
    expect(
      mediaInfoRows({
        status: 'partial',
        container: 'mp4',
        video: null,
        audio: [],
        subtitles: [],
        labels: { video: null, audio: null, channels: null, dynamic_range: null },
      }),
    ).toEqual([
      { label: 'Gelesen', value: 'teilweise' },
      { label: 'Container', value: 'MP4' },
      { label: 'Video', value: 'keine Videospur' },
      { label: 'Audio', value: 'keine Tonspur' },
      { label: 'Untertitel', value: 'keine' },
    ]);
  });

  it('shows SDR without a dynamic range label and unknown values as they are', () => {
    const rows = mediaInfoRows({
      status: 'damaged',
      container: 'avi',
      video: { codec: 'h264', encoder: null, width: null, height: null },
      audio: [],
      subtitles: [],
      labels: { video: 'h264', audio: null, channels: null, dynamic_range: null },
    });
    expect(rows.slice(0, 3)).toEqual([
      { label: 'Gelesen', value: 'beschädigt' },
      { label: 'Container', value: 'avi' },
      { label: 'Video', value: 'h264 · SDR' },
    ]);
  });
});
