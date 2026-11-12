import type { StatusEntry, StatusPoint, StatusSortOrder, StatusGrouping } from '../shared/types';

type Row =
  | { key: string; kind: 'project'; folder: string }
  | { key: string; kind: 'branch'; branch: string }
  | {
      key: string;
      kind: 'point';
      entry: StatusEntry;
      point: StatusPoint;
      pointIndex: number;
      last: boolean;
    };
export type ViewerRow = Row;

function pointIdentity(point: StatusPoint) {
  if (point.id) return `id:${point.id}`;
  return JSON.stringify([point.submitted_at ?? null, point.text, point.done, point.link]);
}

export function pointKey(entry: StatusEntry, point: StatusPoint, pointIndex: number) {
  if (point.id)
    return JSON.stringify(['point', entry.folder_name, entry.session_id, entry.branch, point.id]);
  const identity = pointIdentity(point);
  // A write assigns the same timestamp to multiple points. Include content and
  // an occurrence number so even identical points have distinct, stable keys.
  const occurrence = entry.points
    .slice(0, pointIndex)
    .filter((other) => pointIdentity(other) === identity).length;
  return JSON.stringify([
    'point',
    entry.folder_name,
    entry.session_id,
    entry.branch,
    identity,
    occurrence,
  ]);
}

// All moving rows must be siblings. Animating both a group and its children
// applies the same viewport displacement more than once.
export function viewerRows(
  entries: StatusEntry[],
  projects: string[],
  order: StatusSortOrder = 'submitted-at-desc',
  grouping: StatusGrouping = 'project-directory',
): Row[] {
  const groupKey = (entry: StatusEntry) => groupingKey(entry, grouping);
  return projects.flatMap((group) => {
    const projectEntries = entries.filter((entry) => groupKey(entry) === group);
    const label = projectEntries[0] ? groupLabel(projectEntries[0], grouping) : group;
    const rows: Row[] = [
      { key: JSON.stringify(['project', grouping, group]), kind: 'project', folder: label },
    ];
    for (const branch of new Set(projectEntries.map((entry) => entry.branch))) {
      const repository = projectEntries.find(
        (entry) => entry.branch === branch && entry.repository,
      )?.repository;
      rows.push({
        key: JSON.stringify(['branch', grouping, group, branch]),
        kind: 'branch',
        branch:
          grouping === 'project-directory' && repository ? `${repository} · ${branch}` : branch,
      });
      const points = projectEntries
        .filter((entry) => entry.branch === branch)
        .flatMap((entry) =>
          entry.points.map((point, pointIndex) => ({ entry, point, pointIndex })),
        );
      points.sort((left, right) => {
        const leftTime = Date.parse(left.point.submitted_at ?? left.entry.updated_at);
        const rightTime = Date.parse(right.point.submitted_at ?? right.entry.updated_at);
        return order === 'submitted-at-asc' ? leftTime - rightTime : rightTime - leftTime;
      });
      points.forEach((item, index) =>
        rows.push({
          key: pointKey(item.entry, item.point, item.pointIndex),
          kind: 'point',
          ...item,
          last: index === points.length - 1,
        }),
      );
    }
    return rows;
  });
}

function groupLabel(entry: StatusEntry, grouping: StatusGrouping) {
  return grouping === 'repository' ? entry.repository || entry.folder_name : entry.folder_name;
}

function groupingKey(entry: StatusEntry, grouping: StatusGrouping) {
  if (grouping === 'project-directory') return entry.folder_name;
  return JSON.stringify([
    entry.repository ? 'repository' : 'directory',
    groupLabel(entry, grouping),
  ]);
}

export function projectNames(
  entries: StatusEntry[],
  order: 'project-directory-asc' | 'updated-at-desc' | 'updated-at-asc',
  grouping: StatusGrouping = 'project-directory',
) {
  const latest = new Map<string, number>();
  const labels = new Map<string, string>();
  for (const entry of entries) {
    const key = groupingKey(entry, grouping);
    labels.set(key, groupLabel(entry, grouping));
    latest.set(key, Math.max(latest.get(key) ?? -Infinity, Date.parse(entry.updated_at)));
  }
  const compare = (left: string, right: string) =>
    labels.get(left)!.toLowerCase().localeCompare(labels.get(right)!.toLowerCase()) ||
    left.localeCompare(right);
  return [...latest.keys()].sort((left, right) => {
    if (order === 'project-directory-asc') return compare(left, right);
    const difference = (latest.get(left) ?? 0) - (latest.get(right) ?? 0);
    return difference
      ? order === 'updated-at-asc'
        ? difference
        : -difference
      : compare(left, right);
  });
}

export function relativeTime(value: string, now: number) {
  const minutes = Math.max(0, Math.floor((now - Date.parse(value)) / 60_000));
  if (!minutes) return 'now';
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const remaining = minutes % 60;
  if (hours < 24) return `${hours}h${remaining ? ` ${remaining}m` : ''}`;
  const days = Math.floor(hours / 24);
  const remainingHours = hours % 24;
  return `${days}d${remainingHours ? ` ${remainingHours}h` : ''}`;
}

// Match backend retention ordering, including ties and legacy timestamp fallback.
// At capacity, the oldest item is next to go; also flag any pending overflow.
export function countExpiryKeys(entries: StatusEntry[], limit: number | null): Set<string> {
  const keys = new Set<string>();
  if (limit == null || limit < 1) return keys;
  const projects = new Map<
    string,
    { key: string; time: number; entryIndex: number; pointIndex: number }[]
  >();
  entries.forEach((entry, entryIndex) => {
    const points = projects.get(entry.folder_name) ?? [];
    entry.points.forEach((point, pointIndex) => {
      points.push({
        key: pointKey(entry, point, pointIndex),
        time: Date.parse(point.submitted_at ?? entry.updated_at),
        entryIndex,
        pointIndex,
      });
    });
    projects.set(entry.folder_name, points);
  });
  for (const points of projects.values()) {
    points.sort(
      (a, b) => a.time - b.time || a.entryIndex - b.entryIndex || a.pointIndex - b.pointIndex,
    );
    for (const point of points.slice(0, Math.max(0, points.length - limit + 1)))
      keys.add(point.key);
  }
  return keys;
}
