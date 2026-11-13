// The media info of a library file in German rows (spec web-ui-library AC-05, spec
// mediainfo): read state, container, video, audio and subtitles, in the words the file
// names use (the server's `labels`).

import { kindText, languageName } from '../subtitles/subtitles';

export type MediaInfoRow = { readonly label: string; readonly value: string };

const STATUS: Readonly<Record<string, string>> = {
  complete: 'vollständig',
  partial: 'teilweise',
  damaged: 'beschädigt',
  unsupported: 'Format nicht unterstützt',
  unreadable: 'nicht lesbar',
};

const CONTAINER: Readonly<Record<string, string>> = {
  matroska: 'Matroska',
  webm: 'WebM',
  mp4: 'MP4',
  quick_time: 'QuickTime',
};

type Fields = Readonly<Record<string, unknown>>;

const isRecord = (value: unknown): value is Fields =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const text = (value: unknown): string | null =>
  typeof value === 'string' && value !== '' ? value : null;

const records = (value: unknown): readonly Fields[] =>
  Array.isArray(value) ? value.filter(isRecord) : [];

function videoText(video: unknown, labels: Fields): string {
  if (!isRecord(video)) return 'keine Videospur';
  const size =
    typeof video.width === 'number' && typeof video.height === 'number'
      ? `${video.width} × ${video.height}`
      : null;
  const parts = [
    text(labels.video) ?? text(video.codec),
    size,
    text(labels.dynamic_range) ?? 'SDR',
  ];
  return parts.filter((part) => part !== null).join(' · ');
}

function audioText(labels: Fields): string {
  const parts = [text(labels.audio), text(labels.channels)].filter((part) => part !== null);
  return parts.length === 0 ? 'keine Tonspur' : parts.join(' ');
}

function subtitleText(subtitles: readonly Fields[]): string {
  if (subtitles.length === 0) return 'keine';
  return subtitles
    .map(
      (s) =>
        `${languageName(text(s.language))} (${kindText(s.forced === true, s.hearing_impaired === true)})`,
    )
    .join(', ');
}

export function mediaInfoRows(info: Readonly<Record<string, unknown>>): readonly MediaInfoRow[] {
  const labels = isRecord(info.labels) ? info.labels : {};
  const status = text(info.status);
  const container = text(info.container);
  const audio = records(info.audio);
  const rows: MediaInfoRow[] = [];
  if (status !== null) rows.push({ label: 'Gelesen', value: STATUS[status] ?? status });
  if (container !== null) {
    rows.push({ label: 'Container', value: CONTAINER[container] ?? container });
  }
  rows.push({ label: 'Video', value: videoText(info.video, labels) });
  rows.push({ label: 'Audio', value: audioText(labels) });
  if (audio.length > 0) {
    rows.push({
      label: 'Audiosprachen',
      value: audio.map((a) => languageName(text(a.language))).join(', '),
    });
  }
  rows.push({ label: 'Untertitel', value: subtitleText(records(info.subtitles)) });
  return rows;
}
