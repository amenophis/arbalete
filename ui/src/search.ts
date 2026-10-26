// Whether any of `texts` holds what is typed in a library section's search field, whatever the
// case.
export function matches(query: string, ...texts: string[]): boolean {
  const wanted = query.trim().toLowerCase();
  return texts.some((text) => text.toLowerCase().includes(wanted));
}
