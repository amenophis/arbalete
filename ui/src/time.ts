// "Today", "Yesterday", then the date, for lists grouped by day.
export function day(time: number, now: number): string {
  const start = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const days = Math.round(
    (start(new Date(now)).getTime() - start(new Date(time)).getTime()) / 86_400_000,
  );
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  return new Date(time).toLocaleDateString(undefined, { dateStyle: "long" });
}

// The items of a list, most recent first, grouped under the day `time` gives each.
export function byDay<T>(items: T[], time: (item: T) => number, now: number): [string, T[]][] {
  const days = new Map<string, T[]>();
  for (const item of items) {
    const name = day(time(item), now);
    days.set(name, [...(days.get(name) ?? []), item]);
  }
  return [...days];
}

// The address's host, without "www.".
export function host(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}
